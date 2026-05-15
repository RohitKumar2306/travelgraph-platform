# TravelGraph GraphQL Conventions

- Type names use PascalCase.
- Field and argument names use camelCase.
- Types and fields should include descriptions in published SDL.
- Entity types use Apollo Federation `@key`.
- Mutations return payload unions named `*Payload`.
- Mutation errors are modelled as typed payload union members, not only GraphQL errors.
- Removals require prior deprecation and field usage review.
