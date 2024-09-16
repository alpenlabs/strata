# Instructions for Adding New Guest Code

**1. Create a New Crate**
- Add a new binary crate inside this directory.
- Ensure that the new crate includes `risc0-zkvm` as a dependency.

**2. Update Cargo Configuration**
- Add the new crate to the `package.metadata.risc0` section of the [Cargo.toml file](./Cargo.toml#L24).

**3. [Optional], create a placeholder ELF**
- Create an empty ELF [here](./build.rs#L15). 
- This can assist with linting during development.

**4. Write Tests**
- Implement the necessary tests inside the tests folder.