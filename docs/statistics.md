# Quaderno — statistics (Insights)

**Scope:** version 1.1. Not part of 1.0; kept here so 1.0 doesn't make choices that block it. **Last updated:** 2026-09-28.

## 1. General rules

- **Unit of time:** the journal day (vault spec §5.4).
- **What counts:** entries with `deleted_at` NULL. Links with `deleted_at` NULL. Hidden choices still count (hiding only affects pickers).
- **Quick notes** appear only in the open/expanded counter (§6). They never contribute to any other number.
- **Dreams:** their `mood` is mood on waking. It is never mixed into daily mood figures; it has its own series.
- **Missing values** are skipped, never treated as zero. Every figure shows how many entries it is based on.
- **Minimum sample:** associations (§4) are shown only when based on 5 or more journal days.
- **Filters:** period (week, month, year, all time) and type (journal & dreams, journal, dreams). All figures on screen follow them.
- **Computation:** in memory after unlock, never cached to disk (hard rule 1).

## 2. Summary figures

| Figure | Definition |
|---|---|
| Entries | Count of journal pages + dreams in the period (per type filter) |
| Days written | Distinct journal days with at least one journal page or dream / days in period |
| Longest streak | Longest run of consecutive days written within the period |
| Average mood | Mean `mood` of journal pages in the period |
| Words | Sum of word counts of entries in the period |
| Quick notes | Open count; expanded count; age of the oldest open note |

## 3. Trends

One small chart per rating (mood, energy, cognitive load, sleep), same 1–5 axis.

- **Daily value:** if a journal day has several journal pages with a rating, use their mean.
- **Week and month views:** daily values plus a 7-day trailing average (mean of available values in the last 7 journal days, shown only when at least 3 exist).
- **Year view:** monthly means. **All time:** quarterly means.
- **Dream mood on waking:** a separate optional series in the mood chart, visually distinct and labelled.
- Hover shows the same day on all four charts.

## 4. Associations ("What comes with better days" / "bad days")

For a dimension D (activities, emotions, people, places) and an item X:

- **Baseline:** mean daily mood over all journal days in the period that have a mood value.
- **With X:** mean daily mood over journal days with mood where at least one journal page or dream that day is linked to X.
- **Delta:** With X − Baseline, one decimal. Shown with n = number of those days.
- **Better days** lists items by delta, highest first; **bad days**, lowest first. Up to 8 items.
- Always labelled as association, not cause.

### 4.1 Sleep and next-day energy

For each sleep value s (1–5): mean `energy` on journal day d+1 over days d whose sleep value is s. Sleep for a day = the mean of `sleep` over its journal pages and dreams.

### 4.2 Pattern cards

Short sentences generated from fixed templates, never free text. 1.1 ships these templates:

- "On days with {activity}, {rating} is {higher|lower} by {delta} on average ({n} days)."
- "After {sleep label} sleep, next-day {rating} averages {x} instead of {y} ({n} days)."
- "Your {k} lowest-mood days: {m} had {rating} {label} or worse."

Each card is shown only if its underlying figure meets the minimum sample.

## 5. Recurring themes

- Top 5 people, places, things and tags by number of entries in the period.
- Emotions by count, and colors by share of days.
- **Dreams meet days:** subjects linked to at least one dream and one journal page in the period, with both counts. Dream flag counts (lucid, nightmare, recurring).

## 6. Entry types

- Entries per week by type (journal pages and dreams, side by side).
- Share of entries by type.
- When you write: share of entries by part of day, using the local time of `dated_at` (morning 05:00–12:00, afternoon 12:00–18:00, evening 18:00–05:00).
- Quick notes: open vs expanded counts only.

## 7. Colors in charts

Journal `#c64600` / dark `#ed5b00`; dream `#1c71d8` / dark `#3584e4` (validated for color-vision deficiencies). Ratings use the accent color. Positive associations use the accent color, negative ones a neutral gray; the direction of the bar carries the meaning too.
