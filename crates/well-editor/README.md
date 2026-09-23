# 📜 well-editor: Mneme Inline B-Tree Composition Engine

The `well-editor` crate provides an inline text buffer editor supporting multi-line shell scripting, syntax checking, and fast text manipulation.

---

## Architectural Purpose

When writing complex multi-line loops, functions, or scripts in a terminal, users are typically forced to jump into an external modal editor (`nvim`, `nano`). **Mneme** allows seamless multi-line composition directly at the prompt.

* **Ropey B-Tree Buffer**: Uses an $\mathcal{O}(\log n)$ piece-table / B-tree rope data structure for instantaneous insertions, deletions, and splits on arbitrarily large text buffers.
* **Undo / Redo Stack**: Granular edit history tracking.
* **Cursor & Viewport Math**: Accurate column, line, and visual byte coordinate mapping.

---

## Testing

```bash
cargo test -p well-editor
```
