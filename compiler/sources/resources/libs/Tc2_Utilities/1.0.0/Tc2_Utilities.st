(* Tc2_Utilities compatibility library -- formatting functions.

   This file reproduces the *interface* (name and signature) of the
   LREAL_TO_FMTSTR function from Beckhoff's TwinCAT 3 Tc2_Utilities PLC
   library for interoperability. The body is IronPLC-authored ST implementing
   the normative definition in the clean-room spec at
   specs/design/library-interfaces/tc2-utilities.md (the sole input to this
   implementation): fixed-point decimal formatting with precision clamped to
   0..15, round-half-away-from-zero or truncate-toward-zero, exact digit
   rendering for |in| < 2^63, and the empty string outside that domain.
   Formal parameter names are the vendor's documented names (in, iPrecision,
   bRound) -- not IronPLC's IN/IN1/IN2 convention -- so vendor source using
   formal (named) argument passing resolves unchanged.
   IronPLC is an independent open-source project and is not affiliated with,
   endorsed by, or sponsored by Beckhoff Automation GmbH. See
   specs/steering/compatibility-library-authoring.md. *)

(* Fixed-point decimal rendering of in with exactly LIMIT(0, iPrecision, 15)
   digits after the decimal point; bRound TRUE rounds half away from zero at
   the last place, FALSE truncates toward zero. NaN, +/-infinity, and
   |in| >= 2^63 are out of domain and return ''.

   The digit rendering is deliberately unrolled -- one block per decimal
   weight, highest first -- rather than looped: the VM sizes its temporary
   string-buffer pool by counting string-operation call sites statically and
   releases the buffers only on function return, so a string operation inside
   a loop would exhaust the pool. Unrolled, every call site executes at most
   once per call, which is exactly the model the pool is sized for. *)
FUNCTION LREAL_TO_FMTSTR : STRING
VAR_INPUT
    in         : LREAL;   (* value to format *)
    iPrecision : INT;     (* number of decimal places *)
    bRound     : BOOL;    (* TRUE: round at the last place; FALSE: truncate *)
END_VAR
VAR
    p : INT;              (* precision clamped to 0..15 *)
    i : INT;
    v : LREAL;            (* ABS(in) *)
    intPart : LREAL;      (* integer part of v, exact in binary64 *)
    scale : LREAL;        (* 10^p, exactly representable for p <= 15 *)
    scaled : LREAL;       (* fraction scaled by 10^p *)
    intVal : LINT;        (* integer part as a 64-bit integer *)
    n : LINT;             (* fraction digits as a 64-bit integer *)
    digit : LINT;
    bStarted : BOOL;      (* an integer digit has been emitted *)
    result : STRING;
END_VAR
(* NaN fails every comparison, so it must be tested deliberately: in = in is
   FALSE only for NaN. 9.223372036854775808E18 is 2^63; the >= also catches
   +/-infinity. *)
IF NOT (in = in) OR ABS(in) >= 9.223372036854775808E18 THEN
    LREAL_TO_FMTSTR := '';
    RETURN;
END_IF;

p := LIMIT(0, iPrecision, 15);

(* Both the truncation and the fraction subtraction below are exact in
   binary64. *)
v := ABS(in);
intPart := __TRUNC(v);

scale := 1.0;
FOR i := 1 TO p DO
    scale := scale * 10.0;
END_FOR;
(* The one binary64 rounding in the whole computation. *)
scaled := (v - intPart) * scale;

(* scaled < 10^15 < 2^52, so adding 0.5 is exact; LREAL_TO_LINT truncates
   toward zero. Working on |in| makes bRound round half away from zero
   symmetrically for negative inputs. *)
IF bRound THEN
    n := LREAL_TO_LINT(scaled + 0.5);
ELSE
    n := LREAL_TO_LINT(scaled);
END_IF;

intVal := LREAL_TO_LINT(intPart);
(* The fraction rounded all the way up to 10^p: carry into the integer part.
   Only reachable for v < 2^52 (larger binary64 values are integral), so the
   increment cannot overflow. *)
IF n = LREAL_TO_LINT(scale) THEN
    intVal := intVal + 1;
    n := 0;
END_IF;

(* -0.0 < 0.0 is FALSE, so negative zero renders unsigned. *)
IF in < 0.0 THEN
    result := '-';
ELSE
    result := '';
END_IF;

(* Integer digits by exact 64-bit arithmetic, highest weight (10^18, the
   19th digit -- intVal < 2^63 < 10^19) first, leading zeros suppressed. *)
digit := (intVal / LINT#1000000000000000000) MOD 10;
IF bStarted OR digit <> 0 THEN
    bStarted := TRUE;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
digit := (intVal / LINT#100000000000000000) MOD 10;
IF bStarted OR digit <> 0 THEN
    bStarted := TRUE;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
digit := (intVal / LINT#10000000000000000) MOD 10;
IF bStarted OR digit <> 0 THEN
    bStarted := TRUE;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
digit := (intVal / LINT#1000000000000000) MOD 10;
IF bStarted OR digit <> 0 THEN
    bStarted := TRUE;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
digit := (intVal / LINT#100000000000000) MOD 10;
IF bStarted OR digit <> 0 THEN
    bStarted := TRUE;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
digit := (intVal / LINT#10000000000000) MOD 10;
IF bStarted OR digit <> 0 THEN
    bStarted := TRUE;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
digit := (intVal / LINT#1000000000000) MOD 10;
IF bStarted OR digit <> 0 THEN
    bStarted := TRUE;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
digit := (intVal / LINT#100000000000) MOD 10;
IF bStarted OR digit <> 0 THEN
    bStarted := TRUE;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
digit := (intVal / LINT#10000000000) MOD 10;
IF bStarted OR digit <> 0 THEN
    bStarted := TRUE;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
digit := (intVal / LINT#1000000000) MOD 10;
IF bStarted OR digit <> 0 THEN
    bStarted := TRUE;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
digit := (intVal / LINT#100000000) MOD 10;
IF bStarted OR digit <> 0 THEN
    bStarted := TRUE;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
digit := (intVal / LINT#10000000) MOD 10;
IF bStarted OR digit <> 0 THEN
    bStarted := TRUE;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
digit := (intVal / LINT#1000000) MOD 10;
IF bStarted OR digit <> 0 THEN
    bStarted := TRUE;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
digit := (intVal / LINT#100000) MOD 10;
IF bStarted OR digit <> 0 THEN
    bStarted := TRUE;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
digit := (intVal / LINT#10000) MOD 10;
IF bStarted OR digit <> 0 THEN
    bStarted := TRUE;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
digit := (intVal / LINT#1000) MOD 10;
IF bStarted OR digit <> 0 THEN
    bStarted := TRUE;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
digit := (intVal / LINT#100) MOD 10;
IF bStarted OR digit <> 0 THEN
    bStarted := TRUE;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
digit := (intVal / LINT#10) MOD 10;
IF bStarted OR digit <> 0 THEN
    bStarted := TRUE;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
(* The ones digit always renders: a zero integer part renders '0'. *)
digit := intVal MOD 10;
result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));

(* Fraction: exactly p digits of n, highest weight first -- the block at
   weight 10^k renders only when k < p, which left-pads with zeros to
   exactly p digits. No decimal point when p = 0. *)
IF p > 0 THEN
    result := CONCAT(result, '.');
END_IF;
IF p > 14 THEN
    digit := (n / LINT#100000000000000) MOD 10;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
IF p > 13 THEN
    digit := (n / LINT#10000000000000) MOD 10;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
IF p > 12 THEN
    digit := (n / LINT#1000000000000) MOD 10;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
IF p > 11 THEN
    digit := (n / LINT#100000000000) MOD 10;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
IF p > 10 THEN
    digit := (n / LINT#10000000000) MOD 10;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
IF p > 9 THEN
    digit := (n / LINT#1000000000) MOD 10;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
IF p > 8 THEN
    digit := (n / LINT#100000000) MOD 10;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
IF p > 7 THEN
    digit := (n / LINT#10000000) MOD 10;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
IF p > 6 THEN
    digit := (n / LINT#1000000) MOD 10;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
IF p > 5 THEN
    digit := (n / LINT#100000) MOD 10;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
IF p > 4 THEN
    digit := (n / LINT#10000) MOD 10;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
IF p > 3 THEN
    digit := (n / LINT#1000) MOD 10;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
IF p > 2 THEN
    digit := (n / LINT#100) MOD 10;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
IF p > 1 THEN
    digit := (n / LINT#10) MOD 10;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;
IF p > 0 THEN
    digit := n MOD 10;
    result := CONCAT(result, MID('0123456789', 1, LINT_TO_INT(digit) + 1));
END_IF;

LREAL_TO_FMTSTR := result;
END_FUNCTION
