# Documentation

ida-cli is a CLI-first headless IDA Pro toolkit with an installable skill and an auto-managed local runtime.

## Design

- **CLI-first workflow** - the default user path is `ida-cli`, not a transport protocol
- **Skill-first bootstrap** - the `ida-cli` skill can install and verify the CLI automatically
- **Auto-managed local runtime** - normal use does not require manual server startup
- **Serialized IDA access** - all IDA work still runs through a single worker thread internally

## Contents

- [TOOLS.md](TOOLS.md) - Tool catalog and discovery workflow
- [TRANSPORTS.md](TRANSPORTS.md) - Internal transport and service notes
- [BUILDING.md](BUILDING.md) - Build from source
- [TESTING.md](TESTING.md) - Running tests
