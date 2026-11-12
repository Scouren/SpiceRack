{
  description = "SpiceRack: SpiceRack core rewritten in Rust";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
    # ESPice, built from source: the oracle `espice` the dialect and backend
    # tests run.
    espice.url = "git+https://github.com/OpenFAT/ESPice.git?shallow=1";
    espice.inputs.conformance.follows = "conformance";
    # ESPice at a revision that reads HSPICE: 5f0b8fd (0.3.0), the one every
    # recorded HSPICE result and proof ran on. ESPice is deleting its HSPICE
    # reader (9578555f, the merge of its #263, still has it), and the tests
    # that run HSPICE text run it on ESPice (`espice-hspice
    # --tokenizer=hspice`: tests/test_cross_sim/sim.rs, ZERO-LEDGER P1-063).
    # The revision is in the URL, so `nix flake update` moves `espice` and
    # never this.
    espice-hspice.url = "git+https://github.com/OpenFAT/ESPice.git?shallow=1&rev=5f0b8fda3948d2ad003c87dd4b6710ae89e2dc44";
    espice-hspice.inputs.conformance.follows = "conformance";
    # The VAMS conformance suite: its HSPICE decks and spice-bench's fetch.sh.
    conformance.url = "github:OpenFAT/VAMS-Conformance-Suite";
    conformance.inputs.nixpkgs.follows = "nixpkgs";
    conformance.inputs.flake-utils.follows = "flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, espice, espice-hspice, conformance }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [
          (import rust-overlay)
          # Darwin needs llvmPackages_18 stdenv for ngspice to build.
          # No version pin — nixpkgs-unstable tracks ngspice 44+ which
          # supports OSDI v0.4 modules from openvaf-r.  The binned-model
          # scoped-resolution issue is a fundamental ngspice
          # limitation present in all versions, not a 44-specific regression.
          (final: prev: {
            ngspice = if prev.stdenv.isDarwin
              then prev.ngspice.override { stdenv = prev.llvmPackages_18.stdenv; }
              else prev.ngspice;
          })
        ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };
        pythonEnv = pkgs.python312.withPackages (ps: with ps; [
          numpy
          pytest
          # `pytest -n <N>`: the suite's files run in N processes.
          pytest-xdist
        ]);
        mkOpenvaf = { rev, hash }: pkgs.rustPlatform.buildRustPackage rec {
          pname = "openvaf-r";
          version = "unstable-2026";

          src = pkgs.fetchFromGitHub {
            owner = "arpadbuermen";
            repo = "OpenVAF";
            inherit rev hash;
          };

          cargoLock = {
            lockFile = "${src}/Cargo.lock";
            outputHashes = {
              "salsa-0.17.0-pre.2" = "sha256-6GssvV76lFr5OzAUekz2h6f82Tn7usz5E8MSZ5DmgJw=";
            };
          };

          # Patch build scripts to skip Windows-only steps on Linux hosts.
          # RUST_CHECK was used previously for this but it also skips osdi stdlib.c bitcode
          # generation, producing a broken binary. Instead patch each build.rs individually.
          postPatch = ''
            # target/build.rs: skip MSVC ucrt import-lib on non-Windows host
            sed -i 's/if check {/if check || !cfg!(target_os = "windows") {/g' \
              openvaf/target/build.rs
            # osdi/build.rs: skip generating bitcode for MSVC targets on non-Windows host
            sed -i 's/if no_gen {/if no_gen || (target.options.is_like_windows \&\& !cfg!(target_os = "windows")) {/' \
              openvaf/osdi/build.rs
          '';

          buildAndTestSubdir = "openvaf/openvaf-driver";

          buildFeatures = [ "llvm18" ];

          nativeBuildInputs = with pkgs; [ pkg-config ];

          buildInputs = with pkgs; [
            llvm_18
            libffi
            libxml2
            zlib
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
          ];

          # symlinkJoin provides both llvm-config (for llvm-sys) AND unwrapped clang
          # (for osdi/build.rs stdlib.c -> bitcode cross-compilation).
          # The wrapped clang adds x86_64-specific Nix flags that break -target riscv64 etc.
          env.LLVM_SYS_181_PREFIX = "${pkgs.symlinkJoin {
            name = "llvm18-prefix";
            paths = [
              pkgs.llvm_18.dev
              pkgs.llvmPackages_18.clang-unwrapped
            ];
          }}";

          doCheck = false;

          # Only install the openvaf-r binary
          postInstall = ''
            find $out/bin -type f ! -name "openvaf-r" -delete 2>/dev/null || true
          '';

          meta = {
            description = "OpenVAF-reloaded: Verilog-A compiler for VACASK";
            homepage = "https://github.com/arpadbuermen/OpenVAF";
            license = pkgs.lib.licenses.gpl3Only;
            platforms = pkgs.lib.platforms.linux ++ pkgs.lib.platforms.darwin;
          };
        };

        # ngspice's OSDI 0.4 modules (src/veriloga.rs, the `openvaf` alias).
        openvaf = mkOpenvaf {
          rev = "2e066436d985b05cf8e6563e936daf9ab875775a";
          hash = "sha256-AXtp8qaDq/MRYz2TYXRwT3kS+8EnKyakD3lQwdv3K34=";
        };
        # VACASK's: upstream VACASK builds its device models with the
        # mb-experimental branch (its ci/build-openvaf.sh) and opens no module
        # the older compiler wrote.
        openvafVacask = mkOpenvaf {
          rev = "96b3322cedd9c8b56de4ca009cdeccda6f1ae571";
          hash = "sha256-pFgK7H6EX0dFd8NALIdQ/4lo/iiBECUFl3QjsG0EyXE=";
        };

        # The VACASK deck corpus (golden gate, import suite, cross-simulator
        # cell): the fork's tree the golden baseline recorded, kept apart from
        # the simulator so a VACASK bump leaves the gate's inputs alone.
        vacaskCorpus = pkgs.fetchFromGitHub {
          owner = "robtaylor";
          repo = "VACASK";
          rev = "bcd48e2dd25182f5aaa3392c4e27b4e198372744";
          hash = "sha256-/x6yJ+fklipvYbtI5rHx4d5YIpC9IJ5uhHCtWC5eJJg=";
        };

        vacask = pkgs.stdenv.mkDerivation rec {
          pname = "vacask";
          version = "unstable-2026";

          # Upstream (Arpad Buermen's), not the robtaylor fork pinned before:
          # the fork's vsource validates a `type="pwl"` wave but its
          # sourceCompute has no Pwl case, so every PWL source gave 0 V
          # (lib/devvisrc.cpp); upstream evaluates it.
          src = pkgs.fetchgit {
            url = "https://codeberg.org/arpadbuermen/VACASK";
            rev = "36be560b51c157dbb58ebc0a597deb1c1ba951f7";
            hash = "sha256-dzC795Gf08WLJyBtLQq76DtI52golKPsla3q6XabYTI=";
          };

          nativeBuildInputs = with pkgs; [
            cmake
            ninja
            pkg-config
            python3
            bison
            flex
          ];

          buildInputs = with pkgs; [
            suitesparse
            # LP64: upstream calls the Fortran BLAS (dcopy_, ...) with 32-bit
            # ints; nixpkgs' `openblas` is ILP64 on 64-bit hosts and the first
            # Newton step segfaulted in dcopy_k.
            openblasCompat
            boost
            tomlplusplus
            fftw
          ];

          postPatch = ''
            # --- Bypass the Darwin/Homebrew blocks ---
            # They call brew --prefix and set SuiteSparse_DIR/TOMLPP_DIR/Boost_ROOT
            # to Homebrew paths; renamed, the platform falls through to Linux's.
            sed -i 's/STREQUAL "Darwin"/STREQUAL "DarwinBrew"/g' CMakeLists.txt

            # --- Boost: nixpkgs ships shared libraries only ---
            sed -i 's/set(Boost_USE_STATIC_LIBS ON)/set(Boost_USE_STATIC_LIBS OFF)/' CMakeLists.txt
            sed -i 's/set(Boost_NO_SYSTEM_PATHS TRUE)//' CMakeLists.txt

            # --- Header: nixpkgs' SuiteSparse puts klu.h at the include root ---
            sed -i 's|suitesparse/klu.h|klu.h|g' include/solklu.h

            # --- Portable device models ---
            # OpenVAF compiles for the build machine's CPU by default ('native'). A
            # store path built on one CI runner and fetched from the cache onto a
            # runner with an older CPU then dies with SIGILL in every device model.
            sed -i 's|"''${options_list}" "--allow"|"''${options_list}" "--target_cpu" "generic" "--allow"|' devices/CMakeLists.txt
            grep -q -- '"--target_cpu" "generic"' devices/CMakeLists.txt
          '';

          # Suppress clang warnings that newer clang promotes to hard errors.
          env.NIX_CFLAGS_COMPILE = "-Wno-error -Wno-defaulted-function-deleted";

          cmakeFlags = [
            "-DCMAKE_BUILD_TYPE=Release"
            "-DOPENVAF_DIR=${openvafVacask}/bin"
            "-DTOMLPP_DIR=${pkgs.tomlplusplus}"
            "-DSuiteSparse_DIR=${pkgs.suitesparse}"
          ];

          # CMake's own install: bin/vacask plus lib/vacask/mod/**.osdi (the device
          # models OpenVAF compiled, e.g. spice/capacitor.osdi) and lib/vacask/config,
          # where the binary looks for them relative to itself. Copying only the
          # binary left every netlist that loads a device module unrunnable.

          meta = {
            description = "VACASK – Verilog-A Circuit Analysis Kernel";
            homepage = "https://codeberg.org/arpadbuermen/VACASK";
            license = pkgs.lib.licenses.gpl2Plus;
            platforms = pkgs.lib.platforms.linux ++ pkgs.lib.platforms.darwin;
          };
        };

        # The spice-bench suites (VAMS-Conformance-Suite `external/spice-bench`):
        # its fetch.sh downloads every suite at pinned revisions and converts it to
        # ngspice decks. That needs the network, so it runs in a fixed-output
        # derivation whose hash pins the whole tree. The converted decks carry
        # absolute `.include` paths into the tree; a fixed output cannot name its
        # own store path, so they are written as @SPICE_BENCH@ and filled in by a
        # second, ordinary derivation.
        spiceBench = let
          fetched = pkgs.stdenvNoCC.mkDerivation {
            name = "spice-bench-fetched";
            src = "${conformance}/external/spice-bench";
            nativeBuildInputs = with pkgs; [
              bzip2
              cacert
              curl
              gawk
              git
              gnutar
              gzip
              p7zip
              python3
              unzip
              zstd
            ];
            dontConfigure = true;
            dontInstall = true;
            dontFixup = true;
            buildPhase = ''
              cp -r $src sb && chmod -R u+w sb && patchShebangs sb
              # The scripts call `nix shell nixpkgs#X -c`; X is on PATH here.
              sed -i 's/nix shell nixpkgs#[a-z0-9]* -c //' sb/*/fetch.sh
              export HOME=$TMPDIR SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt
              sb/fetch.sh "$TMPDIR/out"
              # Git metadata is not content: its packs differ run to run.
              find "$TMPDIR/out" -name .git -prune -exec rm -rf {} +
              grep -rlZ --binary-files=without-match "$TMPDIR/out" "$TMPDIR/out" | xargs -0 -r sed -i "s|$TMPDIR/out|@SPICE_BENCH@|g"
              mv "$TMPDIR/out" $out
            '';
            outputHashMode = "recursive";
            outputHash = "sha256-g3Gw1U9aqLcofpHteGPurGNGxxRo8UsPi49J+cqhT/s=";
          };
        in
        pkgs.runCommand "spice-bench" { } ''
          cp -r ${fetched} $out
          chmod -R u+w $out
          grep -rlZ --binary-files=without-match @SPICE_BENCH@ $out | xargs -0 -r sed -i "s|@SPICE_BENCH@|$out|g"
        '';

        # The sky130 and gf180mcu PDKs the examples run against, laid out as a ciel
        # PDK_ROOT (`sky130A/`, `gf180mcuD/`, ...). Only the archives the ngspice
        # model libraries need: each family's `common` (libs.tech) and its primitive
        # devices (`*_fd_pr`, libs.ref). The versions are the ciel releases
        # (fossi-foundation/ciel-releases), pinned by content hash.
        pdks = let
          release = tag: file: hash: pkgs.fetchurl {
            url = "https://github.com/fossi-foundation/ciel-releases/releases/download/${tag}/${file}";
            inherit hash;
          };
          sky130 = "sky130-8afc8346a57fe1ab7934ba5a6056ea8b43078e71";
          gf180 = "gf180mcu-1e3554706ddbb2529193bdabbc7128296f3cb44a";
          archives = [
            (release sky130 "common.tar.zst" "sha256-vopUg0H9kXevnkYa17T92Jr74D26aNxC0VUyH9w/hr8=")
            (release sky130 "sky130_fd_pr.tar.zst" "sha256-DOTQNwGnUO4NaX0DU2yy4+yJyMf0yjfRp3NtVGh0+uY=")
            (release gf180 "common.tar.zst" "sha256-WFQYetcapWoYSuypweZR/0ekSEHutUlS4VdU2qbspDA=")
            (release gf180 "gf180mcu_fd_pr.tar.zst" "sha256-wT2bul+z1FT/7n7q6yFdF5h8kMmBu/sLVyWGNrp8OMw=")
          ];
        in
        pkgs.runCommand "spicerack-pdks" { nativeBuildInputs = [ pkgs.zstd ]; } ''
          mkdir -p $out
          for a in ${pkgs.lib.concatStringsSep " " archives}; do
            zstd -dc "$a" | tar -x -C $out
          done
        '';
        # src/veriloga.rs runs `openvaf`; the reloaded fork installs `openvaf-r`.
        openvafAlias = pkgs.runCommand "openvaf-alias" { } ''
          mkdir -p $out/bin && ln -s ${openvaf}/bin/openvaf-r $out/bin/openvaf
        '';
        # Read-only deck corpora for tests/test_import_suite/'s fixpoints.
        # Xyce 7.10 with Verilog-A plugin support (ZERO D4): ADMS's
        # `admsXml` and `buildxyceplugin.sh`, which compiles a `.va` into a
        # library `Xyce -plugin` loads. nixpkgs' release and sources, built as
        # shared libraries; its manuals and its own regression run left out
        # (tests/test_cross_sim runs the corpus on it).
        xyce = (pkgs.xyce.override { enableDocs = false; enableTests = false; }).overrideAttrs (o: {
          cmakeFlags = (o.cmakeFlags or [ ]) ++ [ "-DXyce_PLUGIN_SUPPORT=ON" "-DBUILD_SHARED_LIBS=ON" ];
          nativeBuildInputs = o.nativeBuildInputs ++ [ pkgs.adms ];
        });
        xyceRegression = pkgs.fetchFromGitHub {
          owner = "Xyce";
          repo = "Xyce_Regression";
          rev = "bbde402756cdf542627ff7ea0056db130fcb005b";
          sparseCheckout = [ "Netlists" ];
          hash = "sha256-D+eOS9tMNZ9L/xVOuQnL2ahIO2zhIiv1YiucOWjAAng=";
        };
        spiceDatasets = pkgs.fetchFromGitHub {
          owner = "symbench";
          repo = "spice-datasets";
          rev = "a3a1c4fc3b8d3d5cd4e986b50e85bd5858751559";
          hash = "sha256-sf1d3jLmCPNg7yphMedOlCtGTUQCeyxPu4hdRdgzkQQ=";
        };

        srcFiltered = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter = path: type:
            let baseName = baseNameOf (toString path); in
            !(baseName == "target" || baseName == "target_nix" || baseName == "result"
              || baseName == ".git");
        };

        spicerackRs = pkgs.python312Packages.buildPythonPackage {
          pname = "spicerack";
          version = "0.1.0";
          format = "pyproject";
          src = srcFiltered;

          cargoDeps = pkgs.rustPlatform.fetchCargoVendor {
            name = "spicerack-vendor";
            hash = "sha256-qFU/ZV+pq5eGHmRtg7gTFSQF93T3IAHHH63DZzeO01k=";
            src = srcFiltered;
          };

          nativeBuildInputs = [
            pkgs.rustPlatform.cargoSetupHook
            pkgs.rustPlatform.maturinBuildHook
            pkgs.cargo
            rustToolchain
            pkgs.pkg-config
          ];

          propagatedBuildInputs = [
            pkgs.python312Packages.numpy
          ];

          pythonImportsCheck = [ "spicerack" ];
        };
        # `nix build .#spice2rack` (ZERO D55): the converter alone, the crate
        # with no default features (the transpiler: no Python binding, no
        # simulator backend), on the dev shell's toolchain. ESPice calls it
        # to read every dialect but ngspice's, so it takes nothing from the
        # `espice` input: only the testing shells do. The crates come from
        # `Cargo.lock`'s own checksums, so there is no vendor hash to follow.
        spice2rack = (pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        }).buildRustPackage {
          pname = "spice2rack";
          version = (pkgs.lib.importTOML ./Cargo.toml).package.version;
          src = srcFiltered;
          cargoLock.lockFile = ./Cargo.lock;
          buildNoDefaultFeatures = true;
          cargoBuildFlags = [ "--bin" "spice2rack" ];
          # The suites run in `.#testing`: they need the simulators.
          doCheck = false;
          meta.mainProgram = "spice2rack";
        };
        # LTspice 26.1.1 (`.#testing-ltspice` only): the MSI unpacked by
        # msitools, run as `ltspice -b deck.cir` under Wine in a headless X
        # server, in a Wine prefix of its own; a `/path` argument goes in as
        # its `Z:` drive path.
        ltspice = let
          msi = pkgs.requireFile {
            name = "LTspice64.msi";
            sha256 = "f22163f15420e43a2d5c66440e038ef031a9f019a181f396b95edcef5eebc357";
            message = ''
              LTspice 26.1.1's installer is not redistributable. Download
              LTspice64.msi from analog.com and add it to the store:
                nix-store --add-fixed sha256 LTspice64.msi
            '';
          };
          app = pkgs.runCommand "ltspice-26.1.1" { nativeBuildInputs = [ pkgs.msitools ]; } ''
            mkdir x && cd x && msiextract ${msi} > /dev/null
            cp -r 'APPDIR:.' $out
          '';
        in pkgs.writeShellApplication {
          name = "ltspice";
          runtimeInputs = [ pkgs.wineWow64Packages.stable pkgs.xvfb-run pkgs.util-linux ];
          text = ''
            export WINEPREFIX="''${XDG_CACHE_HOME:-$HOME/.cache}/spicerack-ltspice"
            export WINEDEBUG=-all WINEDLLOVERRIDES="mscoree,mshtml="
            unset WAYLAND_DISPLAY
            # One run at a time in the one Wine prefix: its first run sets it
            # up, and the `wineserver -k` that ends a run would end another's
            # server. (Each run has a display of its own: `xvfb-run -d`.)
            # ponytail: serialized (a run is about 2 s); a prefix per run if
            # the tests ever run many LTspice decks.
            mkdir -p "$WINEPREFIX"
            exec 9> "$WINEPREFIX.lock"
            flock 9
            ini="$WINEPREFIX/drive_c/users/$(id -un)/AppData/Roaming/LTspice.ini"
            if [ ! -e "$ini" ]; then
              wineboot -i > /dev/null 2>&1
              mkdir -p "$(dirname "$ini")"
              # Without it the first run asks about analytics in a dialog
              # and never returns.
              printf '[Options]\r\nCaptureAnalytics=false\r\n' > "$ini"
            fi
            args=()
            for a in "$@"; do
              case $a in
                /*) args+=("Z:$a") ;;
                *) args+=("$a") ;;
              esac
            done
            # `-d`: Xvfb picks a free display and reports it once it listens,
            # so there is nothing to wait for (`-w 0`; `-a` waited 3 s).
            rc=0
            xvfb-run -d -w 0 wine ${app}/LTspice.exe "''${args[@]}" || rc=$?
            # The Wine server outlives its last client by 3 s and holds fd 9, so the
            # next run would wait for it: end it while this run still has the lock.
            wineserver -k || true
            exit "$rc"
          '';
        };
        # The pinned ESPice's `espice` under its own name, `espice-hspice`
        # (ZERO-LEDGER P1-063): the tests that run HSPICE text keep their
        # oracle when `espice` moves past the HSPICE reader's removal.
        espiceHspice = pkgs.writeShellScriptBin "espice-hspice" ''
          exec ${espice-hspice.packages.${system}.espice}/bin/espice "$@"
        '';
        # Build and unit-test shell. No simulators: SpiceRack emits netlists and
        # runs whatever simulator the user has installed; it never ships one.
        dev = pkgs.mkShell {
          packages = [
            rustToolchain
            pkgs.maturin
            # `cargo nextest run`: every test binary's tests in one pool, not
            # one binary after another (.config/nextest.toml).
            pkgs.cargo-nextest
            pythonEnv
            pkgs.pkg-config
          ];
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          # ponytail: venv pip wheels (numpy) are manylinux binaries needing
          # libstdc++.so.6, which NixOS doesn't expose globally. Upgrade if
          # the venv ever moves fully onto nixpkgs' python package set.
          LD_LIBRARY_PATH = "${pkgs.stdenv.cc.cc.lib}/lib";
        };
      in {
        devShells.default = dev;

        # `nix develop .#testing`: dev shell plus the simulators and PDKs the
        # integration tests check our output against.
        devShells.testing = pkgs.mkShell {
          inputsFrom = [ dev ];
          packages = [
            # ngspice-45 with its periodic steady-state analysis (`.pss`,
            # manual §11.3.12), off in the default build: ZERO D5.
            (pkgs.ngspice.overrideAttrs (o: {
              configureFlags = (o.configureFlags or [ ]) ++ [ "--enable-pss" ];
            }))
            openvaf
            openvafAlias
            vacask
            espice.packages.${system}.espice
            espiceHspice
            # tests/test_cross_sim/: the Xyce corpus on its own simulator and
            # the decks written for Xyce (`xyce` above, pinned by flake.lock);
            # cmake builds its Verilog-A plugins (tests/test_backends.rs p1_039).
            xyce
            pkgs.cmake
            # tests/test_structural.rs: netgen's LVS on the CDL written from a
            # deck, Icarus Verilog on the Verilog.
            pkgs.netgen-vlsi
            pkgs.iverilog
          ];
          # tests/test_import_suite/: the spice-bench round trip and the
          # per-dialect fixpoint corpora.
          # VACASK compiles a loaded Verilog-A source with this compiler, the one
          # its own device models were built with (PATH's openvaf-r is ngspice's).
          SIM_OPENVAF = "${openvafVacask}/bin/openvaf-r";
          SPICE_BENCH = spiceBench;
          NGSPICE_CORPUS = spiceBench;
          HSPICE_CORPUS = "${conformance}/tests/netlist";
          XYCE_CORPUS = "${xyceRegression}/Netlists";
          VACASK_CORPUS = vacaskCorpus;
          LTSPICE_CORPUS = spiceDatasets;
          # tests/dialect_ngspice_corpus.rs (nightly, --features corpus): the example decks of
          # the same pinned ngspice the shell runs, its source unpacked (no new input).
          NGSPICE_EXAMPLES = "${pkgs.srcOnly pkgs.ngspice}/examples";

          # examples 20 and 21: sky130 and gf180mcu, a ciel-layout PDK_ROOT.
          PDK_ROOT = pdks;
        };

        # `nix develop .#testing-ltspice` (ZERO D40): `.#testing` plus LTspice
        # 26.1.1 as `ltspice` (tests/test_backends.rs p1_018). The vendor MSI
        # is not fetched: add it once with
        # `nix-store --add-fixed sha256 LTspice64.msi`. CI runs `.#testing`
        # with the marker LTSPICE_CI_PENDING_MIRROR until the MSI is mirrored.
        devShells.testing-ltspice = self.devShells.${system}.testing.overrideAttrs (o: {
          nativeBuildInputs = o.nativeBuildInputs ++ [ ltspice ];
        });

        # `nix develop .#docs -c mkdocs build`: the documentation site from docs/.
        devShells.docs = pkgs.mkShell {
          packages = [ (pkgs.python312.withPackages (ps: [ ps.mkdocs ps.mkdocs-material ])) ];
        };

        packages.default = spicerackRs;
        packages.spice2rack = spice2rack;
      }
    );
}
