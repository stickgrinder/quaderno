# Quaderno for GNOME — UI spec

**Scope:** version 1.0. **Last updated:** 2026-09-28.
Behavior is in `docs/product-spec.md`; this document says how it looks and which widgets implement it. Design artboards: `design/*.png` next to this file (exported from the design canvas).

## 1. Principles

- GNOME HIG, stock libadwaita widgets, system accent color and style (light/dark follow the system).
- Interface text uses the system UI font. Only entry text uses the writing font (product spec §8).
- Everything reachable by keyboard; every icon-only button has a tooltip and accessible label.

## 2. Window structure

`AdwApplicationWindow` containing an `AdwToastOverlay`, containing a `GtkStack` with these pages:

| Page | When |
|---|---|
| `welcome` | No vault configured (first run) |
| `unlock` | Vault configured and locked |
| `error` | Vault missing or invalid |
| `main` | Vault open |

### 2.1 Main page layout

```
AdwOverlaySplitView (sidebar: sections)        ← outer
└─ AdwNavigationSplitView (sidebar: entry list, content: editor)
   └─ content: AdwOverlaySplitView (sidebar at end: details panel)
```

| Pane | Artboard | Width |
|---|---|---|
| Sections sidebar | Main (left) | 230 px |
| Entry list | Main (middle) | 352 px |
| Editor | Main (centre) | flexible, text column max 660 px |
| Details panel | Main (right) | 440 px |

The default window is 1520 × 860 and starts with the sections sidebar closed;
the header toggle opens it. Window size and maximized state are remembered per
device. When the details panel is shown as an overlay (below 1290 sp) it is
capped at 340 px, so it fits inside narrow windows instead of clipping.

### 2.2 Breakpoints (`AdwBreakpoint`)

| Window width | Change |
|---|---|
| < 1460 sp | Sections sidebar collapses (shown with the sidebar button). Artboard "Dream". |
| < 1290 sp | Details panel becomes an overlay over the editor instead of a side pane. |
| < 760 sp | Entry list and editor become a navigation stack (list → editor with back button). Details panel opens as an `AdwBottomSheet`. |

Minimum window size: 360 × 480, so the layout also works on phone-sized screens.

## 3. Screens

### 3.1 Welcome (first run)

Artboard: FirstRun. `AdwStatusPage` with the app icon, title "Create your journal", then a boxed list:

- `AdwPasswordEntryRow` × 2 (Passphrase, Repeat passphrase), with a `GtkLevelBar` strength hint under them.
- `AdwActionRow` "Location" with the path as subtitle and a "Change…" button (`GtkFileDialog`).
- `AdwSwitchRow` "Remember passphrase on this device".
- A warning `AdwActionRow` with a check button: "I understand that a forgotten passphrase cannot be recovered".
- Suggested-action button "Create Journal"; flat button "Open Existing Journal…".

### 3.2 Unlock

Artboard: Unlock. `AdwStatusPage` with a lock icon, "Quaderno is locked", the vault file name as description, one `AdwPasswordEntryRow` (Enter unlocks), a suggested-action "Unlock" button, and inline error text on failure. Read-only state after unlock shows an `AdwBanner` at the top of the main page.

### 3.3 Sections sidebar

`GtkListBox` with `navigation-sidebar` style:
- Timeline, Journal pages, Dreams, Quick notes (each with a count).
- Heading "Collections": People, Locations, Things, Tags.
- Header bar: app name, main menu (`GtkMenuButton`: Lock, Preferences, Keyboard Shortcuts, About Quaderno).
- "On this day" and "Insights" are 1.1: don't show them in 1.0.

The sidebar starts closed; the header toggle (F9) opens it.

### 3.4 Entry list

- Header bar: New split button (`AdwSplitButton`), title, search toggle.
- `GtkSearchBar` with `GtkSearchEntry` under the header.
- Filter chips: `AdwToggleGroup` (All / Journal / Dreams / Notes).
- `GtkListView` with section headers (journal day), rows as in the Main artboard. Day headers are smaller and dimmer than the entry titles. The type icon sits in a 32 px rounded badge tinted by type: journal orange `#c64600` (dark style `#ed5b00`), dream blue `#1c71d8` (dark `#3584e4`), note neutral. The selected row uses a neutral grey rather than the system accent.
- Empty states: `AdwStatusPage` ("No entries yet", "No results").

### 3.5 Editor

- Header bar: type badge, dream flag chips (dreams only; `GtkToggleButton`s with `pill` style), entry date button (`GtkMenuButton` with a popover holding `GtkCalendar`, a time field, Today / Yesterday buttons), save state label, details toggle, entry menu (Delete, Expand for notes).
- Info bar for a backdated entry: `AdwBanner`-like `GtkRevealer` with text and "Use creation date" button (artboard Dream).
- Body: `GtkSourceView` inside `GtkScrolledWindow`, centred column (max width from the Line width setting), the bundled `quaderno-markdown` language (a modified GtkSourceView Markdown definition that splits headings by level), a custom style scheme that dims Markdown syntax characters while sizing headings, italicising and enlarging blockquotes and giving code a subtle background, word wrap on, spell checking via `libspelling`. A small `GtkSourceView` subclass draws the dimmed 2px bar at the text's left edge and adds a paragraph margin so the quote itself sits clear of it, since style schemes cannot draw borders or add margins.
- Formatting bar at the bottom: heading, bold, italic, list, quote, link buttons (they insert Markdown), plus the footer text (words · created · updated).
- Quick notes use the same editor with a smaller header (artboard Notes): no date info bar, no dream flags, no details panel except tags, plus the "Expand into entry" split button, an "Expanded into" card when the note has been expanded, and a hint that notes are not counted in Insights. An entry made from a note shows a "From quick note" banner with a link to the note (or "From a deleted note").

### 3.6 Details panel

The end sidebar of an `AdwOverlaySplitView` around the editor (see §2.1), toggled
from the editor header with the details button and Ctrl+Shift+I, closing with the
X in its header. It is shown by default and its visibility is remembered per
device (`details-visible`). `GtkScrolledWindow` with `AdwPreferencesGroup`s (artboard Main, right):

- **Entry date:** row with date button (it opens the editor's calendar popover); caption with created/updated times.
- **How it felt:** four rows, each a label plus a linked box of 5 icon `GtkToggleButton`s, allowing no selection (clicking the active toggle clears it). `AdwToggleGroup` is deliberately not used here: it can only ever keep one toggle active, so it cannot express the empty value a rating needs. The value's label is the row subtitle and the button tooltips. The active button uses the system accent color.
- **Emotions / Day activities:** an `AdwWrapBox` of compact chips (`pill` buttons with icon + label; clicking a chip's remove icon unlinks it, with undo toast), plus an inline "+ Add" chip opening a `GtkPopover` with a `GtkSearchEntry` and a `GtkListView` of choices. The popover stays open so several can be picked; already-linked items are not listed.
- **Colors of the day:** 12 circular `GtkToggleButton`s with the color as background, accessible names = color names.
- **People, places & things, Tags:** one token field per kind, each an `AdwPreferencesRow` in the boxed group so the rows carry the same dividers as "How it felt": the kind icon, then an `AdwWrapBox` of chips followed by an inline `GtkText`, so the text sits beside the chips and wraps only when it runs out of room; with a `GtkPopover` suggestion list (`GtkListView`). Artboard Dream shows the suggestion popover. Suggestions are sorted by use count, show "N entries · last <date>", and end with a "Create …" row; Enter picks, Shift+Enter always creates, Backspace on an empty field removes the last token. A comma commits the typed name (links an exact match, otherwise creates) and keeps focus in the field, so several names can be typed in a row; Tab keeps its normal meaning. The popover is non-modal and the list is not focusable, so typing continues in the field; Up/Down move the highlighted suggestion.

The panel only offers what the entry type allows (vault spec §5.1): journal pages
and dreams get every section, quick notes get tags only, and a read-only vault
disables the whole panel.

### 3.7 Quick notes view

Artboard Notes. The Quick notes section (or the Notes chip) turns the list pane
into the notes view: the title becomes "Quick notes", the All/Journal/Dreams/Notes
chips are replaced by an `AdwToggleGroup` Open / Expanded / All (Open is the
default; a note is *expanded* when at least one live entry was made from it), and
rows become cards (the note text, then a dim "Today, 09:12 #tag #tag" footer)
with no journal-day headers. The editor is as in §3.5; notes get a smaller header
(no date info bar, no dream flags), an "Expand into entry" split button and, when
expanded, an "Expanded into" card, plus a hint that notes are not counted in
Insights. An entry made from a note shows a "From quick note" banner (or "From a
deleted note" if the note was deleted), with a link back.

### 3.8 Collections pages

Selecting a collection in the sidebar turns the list pane into that collection
(People, Locations, Things or Tags): the title becomes the collection's name and
the list shows each item with its entry count and a row menu (Rename…, Merge
into…, Delete…). Delete is only offered for items with no linked entries;
otherwise a toast points at merging. Selecting a row filters the pane to a
timeline of that item's entries, with a back button in the header returning to
the collection. This is a mode of the list pane, not a pushed
`AdwNavigationPage`; multi-select merging is done one row at a time via
"Merge into…".

### 3.9 Preferences

`AdwPreferencesDialog`, three `AdwPreferencesPage`s: General, Entries, Privacy & data (artboards PrefsWriting, PrefsLists, PrefsGeneral).

- Font choice: four selectable cards in a `GtkFlowBox` + live preview label; size `AdwSpinRow`; line width `AdwToggleGroup` (a fraction of the editor width, so the choice always takes effect); spell checking a `GtkMenuButton` whose popover lists the installed dictionaries as check buttons (stored as a comma-separated list); default entry type `AdwComboRow`; day end `AdwComboRow` (00:00–06:00 in 30-minute steps).
- Lists: `AdwPreferencesGroup` per kind with an "Add" button; rows carry a drag handle (drag itself is deferred), an icon button opening the icon picker, the title (translated or custom), a "Used in N entries" subtitle and a "Hidden" badge, and a menu with Rename…, Reset name (defaults), Move up, Move down, Hide / Show and Delete…. Rename opens an `AdwAlertDialog` with an entry (plus Reset for defaults); Delete is only offered for unused items; Add suggests an icon from the new name.
- Icon picker: a `GtkPopover` with a `GtkSearchEntry`, a category `GtkDropDown` ("All icons" plus the Phosphor categories) and a virtualized `GtkGridView`. Matching uses the icon name, the `gnome/data/icon-synonyms.tsv` table, then Phosphor's tags and categories (`gnome/data/phosphor-tags.tsv`); icons already used by another item of the same kind are listed last and the fixed scales are excluded.
- Privacy & data: Remember passphrase (asks for it when enabling, then stores it in the keyring), Lock after (`AdwComboRow`), Lock when the session locks, Change passphrase… (asks for the current and new passphrase, checking the current one against the file and explaining a mismatch or a too-short new one inline; the action stays disabled until valid), journal file `AdwActionRow` with "Move…". Toasts raised while Preferences is open are shown on the dialog, not behind it.
- Backups group: an "Automatic backups" switch (weekly, keeping the last 8), a "Backup folder" row whose "Change…" opens a folder chooser and whose "Default" button (shown only when a custom folder is set) resets it, and "Back up now" with a "Back up" button that prompts for the passphrase.
- Export group: an "Export…" row opening an `AdwAlertDialog` that states the export is not encrypted and offers "Markdown folder…" and "JSON file…"; the first opens a folder chooser, the second a save dialog (`quaderno-journal.json`).

### 3.10 Dialogs

`AdwAlertDialog` for: delete confirmation of collection items, export warning ("This export is not encrypted…"), change passphrase, errors. Deleting an entry uses an undo toast instead of a dialog.

## 4. Keyboard shortcuts

| Shortcut | Action |
|---|---|
| Ctrl+N / Ctrl+Alt+N / Ctrl+Shift+N | New journal page / dream / quick note |
| Ctrl+F | Search |
| Ctrl+I | Toggle details panel |
| Ctrl+L | Lock |
| Ctrl+, | Preferences |
| Ctrl+? | Keyboard shortcuts |
| Ctrl+B / Ctrl+I in editor* | Bold / italic |
| Ctrl+1…3 | Heading level |
| Delete (in list) | Delete entry (with undo) |
| Alt+↑ / Alt+↓ | Previous / next entry |
| F9 | Toggle sections sidebar |

\* Ctrl+I conflicts: inside the editor it means italic; the details panel uses Ctrl+Shift+I there. Keep this consistent in `GtkShortcutsWindow`.

## 5. Icons

- Source: Phosphor Icons 2.1.1, regular weight, from the `@phosphor-icons/core` package. Ship `LICENSE` (MIT) under `gnome/data/icons/phosphor/`.
- A build script converts each used SVG into a GTK symbolic icon named `ph-<name>-symbolic` (fill set to a plain color GTK can recolor; drop `currentColor`), and adds it to the GResource. The whole regular set is vendored and converted, so the picker (M6) needs no new assets; they live under `scalable/actions` in the GResource, because that is the directory the `hicolor` fallback theme that carries resource icons actually scans.
- Stored icon names in the vault are bare Phosphor names (`flower-lotus`); the app maps them to `ph-flower-lotus-symbolic`. Unknown names show `ph-question-symbolic`.
- The icon picker's synonym table lives in `gnome/data/icon-synonyms.tsv`, translated terms included.
- The application icon is `gnome/data/icons/hicolor/scalable/apps/io.github.stickgrinder.Quaderno.svg`, with a symbolic variant in `symbolic/apps`. Both are installed with the app and used for the app id.

## 6. Colors and charts

- Use the system accent color for selection, primary actions and the chip tint. Don't hard-code accent colors.
- Entry type colors (§3.4) and the 12 day colors (vault spec Appendix C) are the only fixed colors.
- Charts are 1.1 (Insights); see `docs/statistics.md`.

## 7. Artboard index

| Artboard | Shows |
|---|---|
| Main | Timeline, journal page editor, details panel (wide window) |
| Dream | Collapsed sidebar, dream flags, backdated info bar, calendar popover, people autocomplete |
| Notes | Quick notes list, expand menu, "Expanded into" card |
| FirstRun, Unlock | Vault creation and unlock |
| PrefsWriting, PrefsLists, PrefsGeneral | Preferences pages |
| Icon | Icon vocabulary |
| Insights, QuickCapture, YearPixels | Not in 1.0 (reference only) |
