//! Reference kernels for the spice-engine skill, each a pure function with a
//! test against hand-computed values or the ngspice source it transcribes.
//! Zig 0.17: `zig test numerics.zig`.

const std = @import("std");
const testing = std.testing;

// ---- limiting (ngspice devsup.c DEVpnjlim / DEVfetlim) ----

pub const Limited = struct { v: f64, limited: bool };

/// Junction-voltage limiting, ngspice DEVpnjlim. `vcrit = vt * ln(vt / (sqrt2 * is))`.
pub fn pnjlim(vnew: f64, vold: f64, vt: f64, vcrit: f64) Limited {
    if (vnew > vcrit and @abs(vnew - vold) > vt + vt) {
        if (vold > 0) {
            const arg = (vnew - vold) / vt;
            return .{ .v = if (arg > 0) vold + vt * (2 + @log(arg - 2)) else vold - vt * (2 + @log(2 - arg)), .limited = true };
        }
        return .{ .v = vt * @log(vnew / vt), .limited = true };
    }
    if (vnew < 0) {
        const floor = if (vold > 0) -vold - 1 else 2 * vold - 1;
        if (vnew < floor) return .{ .v = floor, .limited = true };
    }
    return .{ .v = vnew, .limited = false };
}

pub fn critVoltage(vt: f64, is: f64) f64 {
    return vt * @log(vt / (std.math.sqrt2 * is));
}

/// MOS gate-drive limiting, ngspice DEVfetlim. Sets no limited flag, as in ngspice.
pub fn fetlim(vnew_in: f64, vold: f64, vto: f64) f64 {
    var vnew = vnew_in;
    const vtsthi = @abs(2 * (vold - vto)) + 2;
    const vtstlo = @abs(vold - vto) + 1;
    const vtox = vto + 3.5;
    const delv = vnew - vold;
    if (vold >= vto) {
        if (vold >= vtox) {
            if (delv <= 0) {
                if (vnew >= vtox) {
                    if (-delv > vtstlo) vnew = vold - vtstlo;
                } else vnew = @max(vnew, vto + 2);
            } else if (delv >= vtsthi) vnew = vold + vtsthi;
        } else if (delv <= 0) {
            vnew = @max(vnew, vto - 0.5);
        } else vnew = @min(vnew, vto + 4);
    } else if (delv <= 0) {
        if (-delv > vtsthi) vnew = vold - vtsthi;
    } else {
        const vtemp = vto + 0.5;
        if (vnew <= vtemp) {
            if (delv > vtstlo) vnew = vold + vtstlo;
        } else vnew = vtemp;
    }
    return vnew;
}

test "pnjlim matches hand-computed values (vt = 25 mV, Is = 1e-14 A)" {
    const vt: f64 = 0.025;
    const vc = critVoltage(vt, 1e-14);
    try testing.expectApproxEqAbs(0.7050184564, vc, 1e-9);
    // Forward step from a forward-biased junction: log-compressed.
    try testing.expectApproxEqAbs(0.7159764332, pnjlim(1.0, 0.6, vt, vc).v, 1e-9);
    // From reverse/zero bias: v = vt * ln(vnew / vt).
    try testing.expectApproxEqAbs(0.0922219864, pnjlim(1.0, 0.0, vt, vc).v, 1e-9);
    // Falling step above vcrit.
    try testing.expectApproxEqAbs(0.7945199129, pnjlim(0.72, 0.9, vt, vc).v, 1e-9);
    // |dv| == 2 vt exactly is not limited (strict >); binary-exact values.
    try testing.expect(!pnjlim(1.0625, 1.0, 0.03125, critVoltage(0.03125, 1e-14)).limited);
    // Reverse-bias floors.
    try testing.expectEqual(Limited{ .v = -1.5, .limited = true }, pnjlim(-3.0, 0.5, vt, vc));
    try testing.expectEqual(Limited{ .v = -3.0, .limited = true }, pnjlim(-5.0, -1.0, vt, vc));
    try testing.expectEqual(Limited{ .v = -2.0, .limited = false }, pnjlim(-2.0, -1.0, vt, vc));
}

test "fetlim matches hand-computed values (vto = 0.7 V)" {
    try testing.expectApproxEqAbs(1.2, fetlim(5.0, 0.0, 0.7), 1e-12); // off -> on enters at vto + 0.5
    try testing.expectApproxEqAbs(4.7, fetlim(10.0, 2.0, 0.7), 1e-12); // middle, rising: vto + 4
    try testing.expectApproxEqAbs(15.6, fetlim(20.0, 5.0, 0.7), 1e-12); // deep on: vold + vtsthi
    try testing.expectApproxEqAbs(0.2, fetlim(-3.0, 2.0, 0.7), 1e-12); // middle, falling: vto - 0.5
}

// ---- convergence (ngspice niconv.c NIconvTest, per unknown) ----

pub fn rowConverged(new: f64, old: f64, is_voltage: bool, reltol: f64, vntol: f64, abstol: f64) bool {
    if (std.math.isNan(new)) return false;
    const tol = reltol * @max(@abs(old), @abs(new)) + (if (is_voltage) vntol else abstol);
    return !(@abs(new - old) > tol);
}

test "NIconvTest row test at ngspice defaults" {
    // 1 V node: tol = 1e-3 * 1 + 1e-6.
    try testing.expect(rowConverged(1.0, 1.0 + 1.0e-3, true, 1e-3, 1e-6, 1e-12));
    try testing.expect(!rowConverged(1.0, 1.0 + 1.1e-3, true, 1e-3, 1e-6, 1e-12));
    try testing.expect(!rowConverged(std.math.nan(f64), 0, true, 1e-3, 1e-6, 1e-12));
}

// ---- LTE step bound (ngspice cktterr.c CKTterr, one charge state) ----

pub const Method = enum { trap, gear };

/// Largest step the LTE allows for one charge state. `q[0..order+2]` newest
/// first, `h[0..order+1]` = CKTdeltaOld (h[0] is the step just taken),
/// `i_now`/`i_prev` the companion currents CKTstate0/1[ccap].
pub fn stepBound(q: []const f64, h: []const f64, order: u8, method: Method, i_now: f64, i_prev: f64, reltol: f64, abstol: f64, chgtol: f64, trtol: f64) f64 {
    const gear_coeff = [_]f64{ 0.5, 0.2222222222, 0.1363636364, 0.096, 0.07299270073, 0.05830903790 };
    const trap_coeff = [_]f64{ 0.5, 0.08333333333 };
    const volttol = abstol + reltol * @max(@abs(i_now), @abs(i_prev));
    const chargetol = reltol * @max(@max(@abs(q[0]), @abs(q[1])), chgtol) / h[0];
    const tol = @max(volttol, chargetol);
    var diff: [8]f64 = undefined;
    var dt: [8]f64 = undefined;
    for (0..order + 2) |i| diff[i] = q[i];
    for (0..order + 1) |i| dt[i] = h[i];
    var j: i32 = order;
    while (true) {
        for (0..@intCast(j + 1)) |i| diff[i] = (diff[i] - diff[i + 1]) / dt[i];
        j -= 1;
        if (j < 0) break;
        for (0..@intCast(j + 1)) |i| dt[i] = dt[i + 1] + h[i];
    }
    const factor = switch (method) {
        .gear => gear_coeff[order - 1],
        .trap => trap_coeff[order - 1],
    };
    const del = trtol * tol / @max(abstol, factor * @abs(diff[0]));
    return switch (order) {
        1 => del,
        2 => @sqrt(del),
        3 => std.math.cbrt(del),
        else => @exp(@log(del) / @as(f64, @floatFromInt(order))),
    };
}

test "CKTterr divided difference is the plain (order+1)-th divided difference, no k!" {
    // q(t) = t^3 sampled at uneven times: the 3rd divided difference is exactly 1.
    const t = [_]f64{ 1.0, 0.75, 0.25, 0.0 }; // newest first
    var q: [4]f64 = undefined;
    for (&q, t) |*o, ti| o.* = ti * ti * ti;
    const h = [_]f64{ t[0] - t[1], t[1] - t[2], t[2] - t[3] };
    const reltol = 1e-3;
    const abstol = 1e-12;
    // i_now = i_prev = 0 and q >> chgtol, so tol = chargetol = reltol * 1 / h0.
    const got = stepBound(&q, &h, 2, .trap, 0, 0, reltol, abstol, 1e-14, 7);
    const want = @sqrt(7 * (reltol / h[0]) / (0.08333333333 * 1.0));
    try testing.expectApproxEqRel(want, got, 1e-12);
    // A quadratic charge has zero 3rd difference: the abstol floor binds.
    for (&q, t) |*o, ti| o.* = ti * ti;
    const flat = stepBound(&q, &h, 2, .trap, 0, 0, reltol, abstol, 1e-14, 7);
    try testing.expect(flat > 1e3);
}

// ---- dual numbers for device Jacobians ----

pub fn Dual(comptime N: usize) type {
    return struct {
        const D = @This();
        v: f64,
        d: @Vector(N, f64),

        pub fn con(c: f64) D {
            return .{ .v = c, .d = @splat(0) };
        }
        /// Unknown `u` at value `v`: derivative lane u seeded with 1.
        pub fn probe(comptime u: usize, v: f64) D {
            var d: @Vector(N, f64) = @splat(0);
            d[u] = 1;
            return .{ .v = v, .d = d };
        }
        pub fn add(a: D, b: D) D {
            return .{ .v = a.v + b.v, .d = a.d + b.d };
        }
        pub fn sub(a: D, b: D) D {
            return .{ .v = a.v - b.v, .d = a.d - b.d };
        }
        pub fn mul(a: D, b: D) D {
            return .{ .v = a.v * b.v, .d = @as(@Vector(N, f64), @splat(a.v)) * b.d + @as(@Vector(N, f64), @splat(b.v)) * a.d };
        }
        pub fn scale(a: D, c: f64) D {
            return .{ .v = a.v * c, .d = a.d * @as(@Vector(N, f64), @splat(c)) };
        }
        pub fn exp(a: D) D {
            const e = @exp(a.v);
            return .{ .v = e, .d = a.d * @as(@Vector(N, f64), @splat(e)) };
        }
    };
}

test "Dual: diode current and both terminal partials in one pass" {
    const D = Dual(2);
    const is = 1e-14;
    const vt: f64 = 0.025;
    const va = D.probe(0, 0.65);
    const vc = D.probe(1, 0.05);
    const id = va.sub(vc).scale(1.0 / vt).exp().sub(D.con(1)).scale(is);
    const g = is / vt * @exp(0.6 / vt);
    try testing.expectApproxEqRel(is * (@exp(0.6 / vt) - 1), id.v, 1e-14);
    try testing.expectApproxEqRel(g, id.d[0], 1e-14);
    try testing.expectApproxEqRel(-g, id.d[1], 1e-14);
}

// ---- charge-based vs capacitance-based companion ----

test "charge-based BE conserves charge over a closed voltage cycle; C(v)*dv does not" {
    // Nonlinear cap q(v) = c0 v + c1 v^2, C(v) = c0 + 2 c1 v.
    const c0 = 1e-12;
    const c1 = 5e-13;
    const h = 1e-9;
    const vs = [_]f64{ 0, 0.3, 0.9, 1.0, 0.4, 0.0 };
    var charge_q: f64 = 0; // sum of i*h, charge-based
    var charge_c: f64 = 0; // sum of i*h, capacitance-based
    for (vs[1..], vs[0 .. vs.len - 1]) |v, vp| {
        const qn = c0 * v + c1 * v * v;
        const qp = c0 * vp + c1 * vp * vp;
        charge_q += (qn - qp) / h * h;
        charge_c += (c0 + 2 * c1 * v) * (v - vp) / h * h;
    }
    try testing.expectApproxEqAbs(0.0, charge_q, 1e-27);
    try testing.expect(@abs(charge_c) > 1e-14); // leaked charge
}

// ---- MNA stamping through a scatter tape ----

/// Slot of (r, c) in a CSC pattern, `trash` when either index is ground.
fn findSlot(col_ptr: []const u32, row_idx: []const u32, r: u32, c: u32, trash: u32) u32 {
    if (r == ground or c == ground) return trash;
    for (col_ptr[c]..col_ptr[c + 1]) |p| if (row_idx[p] == r) return @intCast(p);
    unreachable; // the pattern was built from these same terminals
}
const ground = std.math.maxInt(u32);

test "resistor stamps through a slot tape; ground lands in the trash slot" {
    // R1 (g=1) node0-node1, R2 (g=2) node1-gnd. Pattern of the 2x2 matrix, CSC.
    const col_ptr = [_]u32{ 0, 2, 4 };
    const row_idx = [_]u32{ 0, 1, 0, 1 };
    const nnz = row_idx.len;
    const terms = [_][2]u32{ .{ 0, 1 }, .{ 1, ground } };
    const g = [_]f64{ 1, 2 };
    // Build once per topology: 4 slots per two-terminal instance, row-major.
    var slots: [terms.len * 4]u32 = undefined;
    for (terms, 0..) |t, id| {
        for (0..2) |ru| for (0..2) |cu| {
            slots[id * 4 + ru * 2 + cu] = findSlot(&col_ptr, &row_idx, t[ru], t[cu], nnz);
        };
    }
    // Every Newton iteration: zero, then a branch-free scatter.
    var vals: [nnz + 1]f64 = @splat(0);
    for (g, 0..) |gi, id| {
        const s = slots[id * 4 ..][0..4];
        vals[s[0]] += gi;
        vals[s[1]] -= gi;
        vals[s[2]] -= gi;
        vals[s[3]] += gi;
    }
    // [[1, -1], [-1, 3]] in CSC order; the trash cell absorbed the ground terms.
    try testing.expectEqualSlices(f64, &.{ 1, -1, -1, 3 }, vals[0..nnz]);
}
