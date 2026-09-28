# Quaderno — UI spec

**Scope:** version 1.0. **Last updated:** 2026-09-28.
Behavior is in `docs/product-spec.md`; this document says how it looks and which widgets implement it. Design artboards: `docs/design/*.png` (exported from the design canvas).

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
| Sections sidebar | Main (left) | 232 px |
| Entry list | Main (middle) | 352 px |
| Editor | Main (centre) | flexible, text column max 660 px |
| Details panel | Main (right) | 372 px |

### 2.2 Breakpoints (`AdwBreakpoint`)

| Window width | Change |
|---|---|
| < 1400 sp | Sections sidebar collapses (shown with the sidebar button). Artboard "Dream". |
| < 1100 sp | Details panel becomes an overlay over the editor instead of a side pane. |
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

### 3.4 Entry list

- Header bar: New split button (`AdwSplitButton`), title, search toggle.
- `GtkSearchBar` with `GtkSearchEntry` under the header.
- Filter chips: `AdwToggleGroup` (All / Journal / Dreams / Notes).
- `GtkListView` with section headers (journal day), rows as in the Main artboard. Type badge colors: journal orange `#c64600` (dark style `#ed5b00`), dream blue `#1c71d8` (dark `#3584e4`), note neutral.
- Empty states: `AdwStatusPage` ("No entries yet", "No results").

### 3.5 Editor

- Header bar: type badge, dream flag chips (dreams only; `GtkToggleButton`s with `pill` style), entry date button (`GtkMenuButton` with a popover holding `GtkCalendar`, a time field, Today / Yesterday buttons), save state label, details toggle, entry menu (Delete, Expand for notes).
- Info bar for a backdated entry: `AdwBanner`-like `GtkRevealer` with text and "Use creation date" button (artboard Dream).
- Body: `GtkSourceView` inside `GtkScrolledWindow`, centred column (max width from the Line width setting), Markdown language, a custom style scheme that dims Markdown syntax characters, word wrap on, spell checking via `libspelling`.
- Formatting bar at the bottom: heading, bold, italic, list, quote, link buttons (they insert Markdown), plus the footer text (words · created · updated).
- Quick notes use the same editor with a smaller header (artboard Notes): no date info bar, no details panel except tags, plus the "Expand into entry" split button and the "Expanded into" card.

### 3.6 Details panel

`GtkScrolledWindow` with `AdwPreferencesGroup`s (artboard Main, right):

- **Entry date:** row with date button; caption with created/updated times.
- **How it felt:** four rows, each a label + `AdwToggleGroup` of 5 icon toggles, allowing no selection (clicking the active toggle clears it). Tooltips give the value's label.
- **Emotions / Day activities:** `GtkFlowBox` of chips (`pill` buttons with icon + label; clicking a chip's remove icon unlinks it, with undo toast), plus an "Add" chip opening a `GtkPopover` with a `GtkSearchEntry` and a `GtkListView` of choices.
- **Colors of the day:** 12 circular `GtkToggleButton`s with the color as background, accessible names = color names.
- **People, places & things, Tags:** one token field per kind: a `GtkFlowBox` of chips followed by a `GtkText`, with a `GtkPopover` suggestion list (`GtkListView`). Artboard Dream shows the suggestion popover.

### 3.7 Quick notes view

Artboard Notes: list of note cards (`GtkListView` with `card` style rows), `AdwToggleGroup` Open / Expanded / All, editor as in §3.5.

### 3.8 Collections pages

`AdwNavigationPage` with a `GtkListView`: name, entry count, row menu (Rename, Merge into…, Delete). Multi-selection mode (selection button in the header) enables "Merge into…". Selecting a row opens the filtered timeline.

### 3.9 Preferences

`AdwPreferencesDialog`, three `AdwPreferencesPage`s: General, Entries, Privacy & data (artboards PrefsWriting, PrefsLists, PrefsGeneral).

- Font choice: four selectable cards in a `GtkFlowBox` + live preview label; size `AdwSpinRow`; line width `AdwToggleGroup`; spell checking `AdwComboRow`; default entry type `AdwComboRow`; day end `AdwComboRow` (00:00–06:00 in 30-minute steps).
- Lists: `AdwPreferencesGroup` per kind; rows with drag handle, icon button (opens the icon picker popover), title (translated or custom label), subtitle ("Used in N entries" / "Hidden"), rename and hide buttons. Rename opens an `AdwAlertDialog` with an entry and a "Reset to default name" response for defaults.
- Icon picker: `GtkPopover` with `GtkSearchEntry`, a "Matching icons" `GtkGridView`, then "All icons" by category.
- Privacy & data: `AdwSwitchRow`s, `AdwSpinRow` for auto-lock minutes, `AdwButtonRow`s for Change passphrase…, Back up now…, Export…; journal file `AdwActionRow` with "Move…".

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
- A build script converts each used SVG into a GTK symbolic icon named `ph-<name>-symbolic` (fill set to a plain color GTK can recolor; drop `currentColor`), and adds it to the GResource. Only icons referenced by the app or the default lists, plus the full picker set, are shipped.
- Stored icon names in the vault are bare Phosphor names (`flower-lotus`); the app maps them to `ph-flower-lotus-symbolic`. Unknown names show `ph-question-symbolic`.
- The icon picker's synonym table lives in `gnome/data/icon-synonyms.tsv`, translated terms included.

## 6. Colors and charts

- Use the system accent color for selection and primary actions. Don't hard-code accent colors.
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
