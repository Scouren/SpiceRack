//! The catalogue's W0301–W0399 rows, stream S4's (D56: the 1.0.0 lanes; none
//! yet: a `// slot <CODE>` marker each). The last block of `catalogue!`'s
//! list (`mod.rs` chains every block, in code order).
//!
//! Cites: each row's `cite` names its own clause.
//!
//! Contents:
//! - `macro_rules! rows_s4`

/// Appends this block's rows to the rows before it (`$acc`) and passes
/// them on to the next block's macro (`$next`, then `$rest`).
macro_rules! rows_s4 {
    ($next:ident $($rest:ident)* ; $($acc:tt)*) => {
        $next! { $($rest)* ; $($acc)*
            // W0301–W0399: stream S4 (D56), one sub-range per spec writer.
            // W0301–W0320: spec-wn (wrong numbers).
            // slot W0301
            // slot W0302
            // slot W0303
            // slot W0304
            // slot W0305
            // slot W0306
            // slot W0307
            // slot W0308
            // slot W0309
            // slot W0310
            // slot W0311
            // slot W0312
            // slot W0313
            // slot W0314
            // slot W0315
            // slot W0316
            // slot W0317
            // slot W0318
            // slot W0319
            // slot W0320
            // W0321–W0335: spec-mm (cross-sim mismatch triage).
            /// More than one analysis in a deck written for Xyce, which runs one a deck.
            W0321 Refusal {
                title: "more than one analysis, and Xyce runs one a deck",
                cite: "Xyce 7.10.0 run on two-analysis decks (ZERO-LEDGER P1-066): of `.DC`, `.AC`, `.TRAN`, `.NOISE` cards the last alone is run, with no message; two `.DC` cards run as one nested sweep; Xyce RG 7.10 `.OP` (an `.op` beside another analysis is not calculated: W0052)",
                explain: "The deck holds two or more of `.dc`, `.ac`, `.tran`, `.noise`, `.sp`. ngspice, HSPICE and VACASK run each of them; Xyce runs the last card only and reports nothing, and reads two `.DC` cards as one nested sweep, so the written deck would give other results than its source. Write one deck per analysis. An `.op` beside one analysis is kept and noted (W0052).",
            }
            // slot W0322
            // slot W0323
            // slot W0324
            // slot W0325
            // slot W0326
            // slot W0327
            // slot W0328
            // slot W0329
            // slot W0330
            // slot W0331
            // slot W0332
            // slot W0333
            // slot W0334
            // slot W0335
            // W0336–W0375: spec-rf (specific refusals).
            /// A statement kept as text that its own simulator runs to the same
            /// numbers without (a print width, a file request, a line it reads
            /// past): left out off its family.
            W0336 Note {
                title: "a statement that changes no number of the run, not written",
                cite: "ngspice-45 manual §11.6.7 (`.width`: the characters a printed row holds), §2.13 (`.csparam`: a parameter as a vector for `.control` scripts); LTspice help, `.WAVE` (nodes written to a .wav file); Xyce RG 7.10 §2.1.34 (`.SAVE`: the operating point stored in a file), §6.1.4 (\"Xyce does not support the .PROBE statement\"); Xyce 7.10 runs: \"Unrecognized dot line will be ignored\" (`.DCOP`, `.OBJECTIVE`, `.PROBE/CSD`), \"Subcircuit .ENDS without .SUBCKT, ignoring\"; ngspice-45, LTspice 26.1.1 and Xyce 7.10 each give the same numbers with the line and without it",
                explain: "The statement sets how its own simulator prints or files its results (ngspice's `.width` and `.csparam`, LTspice's `.wave`, Xyce's `.SAVE`), or is a line that simulator reads past with a warning (Xyce: `.DCOP`, `.OBJECTIVE`, `.PROBE`, an `.ENDS` with no `.SUBCKT` open). The circuit and its analyses run to the same numbers without it, and it is the source dialect's text, so a target of another family gets the deck without it. Ask the target for the file or the print layout with its own statement (HSPICE has `.WIDTH` and `.SAVE`).",
            }
            // slot W0337
            // slot W0338
            // slot W0339
            // slot W0340
            // slot W0341
            // slot W0342
            // slot W0343
            // slot W0344
            // slot W0345
            // slot W0346
            // slot W0347
            // slot W0348
            // slot W0349
            // slot W0350
            // slot W0351
            // slot W0352
            // slot W0353
            // slot W0354
            // slot W0355
            // slot W0356
            // slot W0357
            // slot W0358
            // slot W0359
            // slot W0360
            // slot W0361
            // slot W0362
            // slot W0363
            // slot W0364
            // slot W0365
            // slot W0366
            // slot W0367
            // slot W0368
            // slot W0369
            // slot W0370
            // slot W0371
            // slot W0372
            // slot W0373
            // slot W0374
            // slot W0375
            // W0376–W0399: spec-tr (translations).
            // slot W0376
            // slot W0377
            // slot W0378
            // slot W0379
            // slot W0380
            // slot W0381
            // slot W0382
            // slot W0383
            // slot W0384
            // slot W0385
            // slot W0386
            // slot W0387
            // slot W0388
            // slot W0389
            // slot W0390
            // slot W0391
            // slot W0392
            // slot W0393
            // slot W0394
            // slot W0395
            // slot W0396
            // slot W0397
            // slot W0398
            // slot W0399
        }
    };
}
