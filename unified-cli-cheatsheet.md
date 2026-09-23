# Unified Terminal Cheatsheet: Ghostty + Fish + Starship
*A high-productivity quick-reference guide for the modern GPU-accelerated terminal ecosystem, compiled from system documentation and release notes.*

---

## 1. Ghostty 1.3.0 Core Keyboard Shortcuts & Controls

Ghostty handles terminal emulator configurations, split-panes, tab controls, and viewport rendering. On macOS, primary operations use `Cmd` [23, 335]. On Linux/GTK, they use `Ctrl+Shift` or `Ctrl` to avoid conflicting with standard terminal control characters [4, 335].

### Split-Pane & Tab Management
| Shortcut (macOS) | Shortcut (Linux/GTK) | Action / Description |
| :--- | :--- | :--- |
| **`Cmd + T`** | **`Ctrl + Shift + T`** / **`Ctrl + T`** [335] | Open a new tab. |
| **`Cmd + W`** | **`Ctrl + Shift + W`** / **`Ctrl + W`** [335] | Close current split pane (or window if it is the last tab). |
| **`Cmd + N`** | **`Ctrl + Shift + N`** / **`Ctrl + N`** [335] | Open a new window. |
| **`Cmd + D`** | **`Ctrl + Shift + D`** / **`Ctrl + D`** [335] | Split active pane horizontally. |
| **`Cmd + ,`** or **`F12`** | **`F12`** [335] | Open the Standalone Settings GUI window. |
| **`Double-Click`** divider | **`Double-Click`** divider [23] | Equalize split-pane widths on the screen. |
| **Drag top handle** | — [23] | Grab the drag handle at the top of a terminal split to reorder splits, or drag it out into a new tab or window. |
| **Right-Click** tab | — [23] | Assign a custom color to a specific tab. |
| **Double-Click** tab | — [23] | In-line edit the tab title. Press `Enter` to confirm, `Esc` to cancel. |
| **Two-Finger Swipe** | **Two-Finger Swipe** [19] | Swipe left or right on a touchpad to switch active tabs. |

### Scrolling & Scrollback Search
| Shortcut (macOS) | Shortcut (Linux/GTK) | Action / Description |
| :--- | :--- | :--- |
| **`Cmd + F`** [4] | **`Ctrl + Shift + F`** [4] | Toggle Scrollback Search bar (concurrent background search thread) [5]. Drag search bar to any of the 4 corners [4, 5]. |
| **`Cmd + G`** [4] | **`Enter`** [4] | Find next match in scrollback search. |
| **`Shift + Cmd + G`** [4] | **`Shift + Enter`** [4] | Find previous match in scrollback search. |
| **`Cmd + Home`** [23] | — | Scroll instantly to the very top of the scrollback history buffer. |
| **`Cmd + End`** [23] | — | Scroll instantly to the very bottom of the scrollback history buffer. |

### Clipboard Operations
* **`Cmd/Ctrl + C`** / **`Cmd/Ctrl + V`**: Copy and Paste [335].
* **Rich Clipboard Copy**: Ghostty automatically copies text to the clipboard setting both `text/plain` and `text/html` formats, preserving terminal colors and syntax formatting when pasted into rich-text applications [11].
* **Copy with Escape Sequences**: You can map `copy_to_clipboard` with the `vt` parameter to copy text including raw ANSI escape codes to preserve styling across terminals [11].

---

## 2. Fish Shell Editing, Navigation & Autosuggestion Shortcuts

Fish serves as the interactive command-line shell, providing out-of-the-box auto-suggestions, tab completions, and programmable prompts [424, 425, 427, 437].

### Auto-Suggestion & Completion Control
* **Accept entire suggestion**: Press **`→` (Right Arrow)** or **`Ctrl + F`** [426]. This inserts the gray-colored auto-suggestion inline [425, 426].
* **Accept next word of suggestion**: Press **`Alt + →`** or **`Alt + F`** [426]. This completes the suggestion up to the next space or path separator, ideal for folder-by-folder directory navigation [426].
* **Accept next token of suggestion**: Press **`Ctrl + →`** (Right Arrow) [442].
* **Accept next big word of suggestion**: Press **`Shift + →`** (ignores punctuation boundaries) [442].
* **Decline suggestion**: Simply ignore it and type normally; it will not execute unless explicitly accepted [426].
* **Trigger Tab Completion**: Press **`Tab`** to guess the rest of a word or open the completion pager menu if there are multiple possibilities [427].
* **Search within Tab Completion Pager**: While the completion menu is active, press **`Ctrl + S`** (or **`/`** in Vi-mode) to filter options instantly [427, 447].

### Shell HistoryPrefix Searching
Fish permits robust historical prefix searching based on what is already entered on the line [442].
* **Search commands matching prefix**: Type a prefix (e.g., `git `) and press **`↑` (Up Arrow)** or **`↓` (Down Arrow)** (or **`Ctrl + P`** / **`Ctrl + N`**) to cycle through matching commands [442].
* **Search arguments/tokens matching prefix**: Type a token (e.g., `-l`) and press **`Alt + ↑`** or **`Alt + ↓`** to search history specifically for matching arguments [442].
* **Toggle interactive History Pager**: Press **`Ctrl + R`** to open your complete command history in a paginated list with real-time fuzzy filtering [444].

### Highly Productive Shared Bindings
* **`Alt + S`**: Prepends **`sudo`** (or `doas`, `please`, `run0`) to your current command line, or prepends it to the previously executed command if the line is empty [443].
* **`Alt + E`** or **`Alt + V`**: Open the current multi-line command buffer in your system editor (defined by `$VISUAL` or `$EDITOR`) [443]. Saving and exiting the editor will load the edited script back into the terminal line [443, 535].
* **`Alt + L`**: Instantly list the contents of the current working directory (or the folder under the cursor) without clearing your active command line [442].
* **`Alt + H`** or **`F1`**: Instantly show the `man` page documentation for the command under your cursor [442].
* **`Alt + O`**: Open the file path currently under your cursor in a terminal pager [442], or open the script under the cursor in your configured text editor [442].
* **`Alt + P`**: Append `&| less;` (or your configured `$PAGER` variable) to the end of the current line to run it through a pager on execution [443].
* **`Alt + W`**: Print a short description of the command currently under your cursor [443].
* **`Ctrl + L`**: Clears the terminal screen [442]. (In Ghostty 1.3.0, the parser correctly preserves scrollback history during `CSI Scroll Up` operations, meaning `Ctrl+L` will push lines up to scrollback instead of erasing them [21]).
* **`Ctrl + Space`**: Inserts a literal space without expanding a Fish Shell abbreviation [443].
* **Skip command history**: Prefix your command with a leading **Space** character to execute it without writing it to your disk history file [418, 455].

### Vi-Mode Modal Controls
Toggle Vi bindings by running `fish_vi_key_bindings` in your shell [441]. Switch back via `fish_default_key_bindings` [441].

* **Insert Mode**: Default mode when starting [445]. Use **`Esc`** to enter Normal/Command Mode [445].
* **Normal Mode Navigation**:
  * **`h`** / **`l`**: Move cursor one character left / right [447].
  * **`k`** / **`j`**: Move cursor up / down (and prefix-search history) [447].
  * **`0`** / **`$`**: Move to the beginning / end of the current line [538].
  * **`w`** / **`b`** / **`e`**: Move forward to start of next word / backward to start of previous word / forward to end of current word [538].
* **Normal Mode Actions**:
  * **`i`** / **`a`**: Switch to Insert Mode at cursor / after cursor [538].
  * **`I`** / **`A`**: Switch to Insert Mode at beginning / end of line [538].
  * **`v`**: Enter Visual Selection Mode [447].
  * **`d,d`**: Delete current line (moves to copy-paste kill ring) [447].
  * **`D`**: Delete from cursor position to end of line [538].
  * **`p`** / **`P`**: Paste text from kill ring after / before current character [538].
  * **`u`**: Undo last change [538].
  * **`Ctrl + R`**: Redo last undone change [447].
  * **`~`**: Toggle character casing (uppercase / lowercase) [447].

---

## 3. Starship Prompt Visual Cues & Environmental Indicators

Starship does not inject hotkeys (it is a prompt generator) [378], but it changes its colors and symbols dynamically to give you real-time environmental context.

### Prompt Character Modes
The character prompt module (usually `❯`) changes both color and shape dynamically:
* **Green `❯`**: Last command executed successfully [167, 168].
* **Red `❯`** or **Red `✖`**: Previous command failed (red denotes error, can be configured to display an error symbol with exit code) [167, 168].
* **Vim Mode Indicators** (when Vi-mode is enabled in your shell) [499]:
  * **Green `❮`**: Shell is in **Vim Normal Mode** [168, 500].
  * **Purple `❮`**: Shell is in **Vim Replace Mode** (or Replace One mode) [168, 500].
  * **Yellow `❮`**: Shell is in **Vim Visual Selection Mode** [168, 500].

### Context-Aware Modules (Lazy Load)
Modules are configured to load lazily, meaning they only take up prompt space if they are relevant to your directory [586].
* **Git Status Modules**: Displays git context such as branch name (`on  main`), ahead/behind count (`⇡` / `⇣`), additions/deletions metrics (`+7/-7`), and ongoing states (`REBASING` / `MERGING`) [497, 574].
* **Language Environment Modules**: Shows runtime versions (e.g., `via  v20.5.0` for Node.js, `via 🐍 v3.11.0` for Python, `via 🦀 v1.75.0` for Rust) only when matching configurations (like `package.json`, `requirements.txt`, or `Cargo.toml`) are in your working directory [574].
* **Cloud & Server Context**: Displays active AWS profiles, Kubernetes contexts (` cluster-name`), or Docker contexts (`🐳 context-name`) to protect you from executing commands in the wrong production environment [573, 575, 586].

### Latency Optimization Settings
Configure these options in your `~/.config/starship.toml` to prevent slow directory scanning (such as massive Git repositories or slow remote mounts) from introducing shell latency [162, 238, 586]:
```toml
# Timeout for scanning directory files (in milliseconds)
scan_timeout = 30

# Timeout for executing language version commands (in milliseconds) 
command_timeout = 500
```

---

## 4. Advanced Power-User Configurations

Combine the capabilities of Ghostty, Fish, and Starship with these high-performance integration configurations.

### Transient Prompts (Keeping Scrollback Clean)
Transient prompts shrink your historical prompts to a single, compact character (like a green `❯`) when you press `Enter`, keeping your terminal scrollback clear of layout decorations [60, 240].

1. Add this to your `~/.config/fish/config.fish` file:
   ```fish
   enable_transience
   ```
2. (Optional) Customize the transient prompt output to show Starship's character module by defining a custom function in `config.fish` [61]:
   ```fish
   function starship_transient_prompt_func
       starship module character
   end
   ```

### Ghostty Chained Keybindings
You can bind multiple sequential actions to a single key combination using the `chain` keyword inside your `~/.config/ghostty/config` file [9]:
```ini
# Toggle native fullscreen and disable window decorations with one press
keybind = ctrl+shift+f=toggle_fullscreen
keybind = ctrl+shift+f=chain:toggle_window_decorations
```

### Ghostty Key Tables (Tmux-style Modal Workflows)
Set up custom, modal command key tables inside Ghostty using named tables [8, 9]. While a key table is active, Ghostty routes inputs to those commands and ignores standard entries [9].
```ini
# Trigger the 'resize' table with Ctrl+A
keybind = ctrl+a=activate_key_table:resize

# Configure actions inside the 'resize' table (arrow keys resize split panes)
keybind = resize:right=resize_split:right
keybind = resize:left=resize_split:left
keybind = resize:up=resize_split:up
keybind = resize:down=resize_split:down

# Escape exits the table and returns to normal terminal input
keybind = resize:escape=pop_key_table
```
