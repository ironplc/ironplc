# Add a New Problem Code

Add a new error code to IronPLC. See [specs/steering/problem-code-management.md](../../specs/steering/problem-code-management.md) for the full problem code lifecycle and patterns.

## Compiler Problem Codes (P-prefix)

### Steps

1. **Find the next available code**: Check `compiler/problems/resources/problem-codes.csv` for the next sequential P#### number
2. **Add to CSV**: Append a row to `compiler/problems/resources/problem-codes.csv`
   ```csv
   Code,Name,Message
   P####,PascalCaseName,Brief description of the error
   ```
3. **Create documentation**: Add `docs/compiler/problems/P####.rst` using the template in the steering doc
4. **Implement usage**: Create a `Diagnostic::problem(Problem::YourName, ...)` in the Rust code
5. **Add tests**: Verify the error is generated with the correct problem code
6. **Run CI**: `cd compiler && just` (all checks must pass)

### Code ranges
- P0001-P1999: Parsing errors
- P2000-P3999: Type system errors
- P4000-P5999: Semantic analysis errors
- P6000-P7999: File system errors
- P9000+: Internal errors

## VS Code Extension Error Codes (E-prefix)

1. Add to `integrations/vscode/resources/problem-codes.csv`
2. Run the build (auto-generates `src/problems.ts`)
3. Create `docs/vscode/problems/E####.rst`
4. Update `docs/vscode/problems/index.rst`
5. Use `formatProblem(ProblemCode.Name, context)` — never hardcode strings

## VM Runtime Error Codes (V-prefix)

1. Choose code in the correct range: V4xxx (execution), V6xxx (IO), V9xxx (internal)
2. Add to the correct CSV: `compiler/vm/resources/problem-codes.csv` (trap codes) or `compiler/vm-cli/resources/problem-codes.csv` (IO codes)
3. Update Rust code: for trap codes, add variant to `Trap` enum in `compiler/vm/src/error.rs`
4. Create `docs/reference/runtime/problems/V####.rst`
5. Add integration test verifying V-code in stderr and exit code
