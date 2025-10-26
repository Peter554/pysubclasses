See @README for a general project overview.

## Additional Instructions

- Write idiomatic rust code that is simple as possible. Avoid excessive abstraction.
- Look for opportunities to refactor, simplify and improve the code.
- Ensure that code is documented, and that the documentation is up to date with the code.
- When you're done making changes test, lint and format the code to check everything.

## Testing Guidelines

- All code should have tests (unit and/or integration tests).
- Use the assert_fs and assert_cmd crates for integration testing.
- Where parametric tests are appropriate, use the yare crate.
  Always create a struct (e.g. `Case`) to hold test case data.
