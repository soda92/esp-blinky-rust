# Rust vs Python vs Go: Imports & Modules

Here is the comparison table explaining how Rust's module system differs from Python and Go.

| Feature | Python | Go | Rust |
| :--- | :--- | :--- | :--- |
| **How to make a module?** | Create a `.py` file. | Create a folder/file with `package name`. | Create a `.rs` file **AND** add `mod filename;` in the parent (e.g., `lib.rs`). |
| **Visibility** | Everything is public (convention uses `_`). | **Capitalized** = Public.<br>**lowercase** = Private. | **Private by default**. Must use `pub` to expose it. |
| **Same folder access** | Must import explicitly. | Automatic (no import needed). | Must import explicitly (via `use`). |
| **Project Structure** | Filesystem driven. | Package driven. | Explicit module tree driven. |

### Key Takeaways for Rust

1.  **The "Tree" vs. The "Filesystem":**
    *   **Python:** The filesystem *is* the module structure. If `mqtt.py` exists, you can `import mqtt`.
    *   **Go:** Files in the same folder (package) automatically see each other.
    *   **Rust:** You must **manually build the module tree**. Even if `src/mqtt.rs` exists, the compiler completely ignores it until you explicitly declare `mod mqtt;` in a parent file (like `lib.rs` or `main.rs`).

2.  **Explicit Visibility:**
    *   In Rust, everything is **private** by default.
    *   To make your `mqtt` module usable by `main.rs`, we had to:
        1.  Declare it in `lib.rs`: `pub mod mqtt;` (The `pub` makes it accessible outside `lib.rs`).
        2.  Import it in `main.rs`: `use esp_blinky_rust::mqtt::{...}`.

3.  **Why Rust does this?**
    *   It decouples the **file structure** from the **API structure**.
    *   You can have a complex folder structure with many files but expose a clean, simple API hierarchy in `lib.rs` by re-exporting only what you want (`pub use internal_mod::Type;`).
