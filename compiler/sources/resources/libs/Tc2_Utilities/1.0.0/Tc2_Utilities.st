(* Tc2_Utilities compatibility library -- utility functions.

   This file reproduces the *interface* of Beckhoff's TwinCAT Tc2_Utilities
   library for interoperability. IronPLC is an independent open-source project
   and is not affiliated with, endorsed by, or sponsored by Beckhoff
   Automation GmbH. Authored from the clean-room spec at
   specs/design/library-interfaces/tc2-utilities.md; see
   specs/steering/compatibility-library-authoring.md.

   LREAL_TO_FMTSTR is declare-only (library.toml): the signature resolves
   during check, and a call is compile error P4046 until the formatting
   implementation lands. The `;` body is never compiled. The vendor return
   type STRING(255) is spelled with the standard bracket form STRING[255],
   which declares the identical 255-character capacity. *)

(* Converts an LREAL to a formatted string: iPrecision decimal places,
   rounded when bRound is TRUE, truncated when FALSE. *)
FUNCTION LREAL_TO_FMTSTR : STRING[255]
VAR_INPUT
    in         : LREAL;
    iPrecision : INT;
    bRound     : BOOL;
END_VAR
;
END_FUNCTION
