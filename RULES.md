# wcag-lsp Rules

wcag-lsp performs static analysis of HTML and JSX/TSX code to detect WCAG 2.1/2.2 accessibility violations. This document lists all 40 rules and maps them to WCAG success criteria.

> **Note:** Many WCAG criteria require runtime testing, visual inspection, or assistive technology and cannot be checked statically. These are documented in the [coverage matrix](#wcag-22-criterion-coverage) below.

## Rule Reference

| Rule ID | WCAG Criterion | Level | Default Severity | Description |
|---------|---------------|-------|-----------------|-------------|
| `anchor-content` | [2.4.4](https://www.w3.org/WAI/WCAG21/Understanding/link-purpose-in-context.html) | A | Error | `<a>` elements must have text content |
| `area-alt` | [1.1.1](https://www.w3.org/WAI/WCAG21/Understanding/non-text-content.html) | A | Error | `<area>` elements must have alt, aria-label, or aria-labelledby |
| `aria-allowed-attr` | [4.1.2](https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html) | A | Error | ARIA attributes must be allowed for the element's role |
| `aria-deprecated-role` | [4.1.2](https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html) | A | Warning | Deprecated ARIA roles must not be used |
| `aria-hidden-body` | [4.1.2](https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html) | A | Error | `<body>` must not have `aria-hidden="true"` |
| `aria-hidden-focus` | [4.1.2](https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html) | A | Error | Elements with `aria-hidden="true"` must not contain focusable elements |
| `aria-prohibited-attr` | [4.1.2](https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html) | A | Error | ARIA attributes prohibited for a role must not be used |
| `aria-props` | [4.1.2](https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html) | A | Error | `aria-*` attributes must be valid ARIA properties |
| `aria-required-attr` | [4.1.2](https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html) | A | Error | Elements with ARIA roles must have all required ARIA attributes |
| `aria-required-children` | [1.3.1](https://www.w3.org/WAI/WCAG21/Understanding/info-and-relationships.html) | A | Error | Elements with ARIA roles must have required child roles |
| `aria-required-parent` | [1.3.1](https://www.w3.org/WAI/WCAG21/Understanding/info-and-relationships.html) | A | Error | Elements with ARIA roles must be contained in required parent roles |
| `aria-role` | [4.1.2](https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html) | A | Error | `role` attribute must be a valid ARIA role |
| `aria-valid-attr-value` | [4.1.2](https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html) | A | Error | ARIA attribute values must be valid for their type |
| `autocomplete-valid` | [1.3.5](https://www.w3.org/WAI/WCAG21/Understanding/identify-input-purpose.html) | AA | Warning | `autocomplete` attribute must have a valid value |
| `button-name` | [4.1.2](https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html) | A | Error | `<button>` elements must have an accessible name |
| `click-events-have-key-events` | [2.1.1](https://www.w3.org/WAI/WCAG21/Understanding/keyboard.html) | A | Error | Elements with `onClick` must also have `onKeyDown` or `onKeyUp` |
| `form-label` | [1.3.1](https://www.w3.org/WAI/WCAG21/Understanding/info-and-relationships.html) | A | Error | Form elements must have associated labels |
| `heading-content` | [2.4.6](https://www.w3.org/WAI/WCAG21/Understanding/headings-and-labels.html) | AA | Warning | Heading elements must have text content |
| `heading-order` | [1.3.1](https://www.w3.org/WAI/WCAG21/Understanding/info-and-relationships.html) | A | Warning | Heading levels should not be skipped |
| `html-lang` | [3.1.1](https://www.w3.org/WAI/WCAG21/Understanding/language-of-page.html) | A | Error | `<html>` element must have a `lang` attribute |
| `iframe-title` | [2.4.1](https://www.w3.org/WAI/WCAG21/Understanding/bypass-blocks.html) | A | Error | `<iframe>` elements must have a `title` attribute |
| `img-alt` | [1.1.1](https://www.w3.org/WAI/WCAG21/Understanding/non-text-content.html) | A | Error | `<img>` elements must have an `alt` attribute |
| `input-image-alt` | [1.1.1](https://www.w3.org/WAI/WCAG21/Understanding/non-text-content.html) | A | Error | `<input type="image">` elements must have an `alt` attribute |
| `lang-valid` | [3.1.1](https://www.w3.org/WAI/WCAG21/Understanding/language-of-page.html) | A | Error | `lang` attribute must have a valid BCP 47 primary language subtag |
| `list-structure` | [1.3.1](https://www.w3.org/WAI/WCAG21/Understanding/info-and-relationships.html) | A | Error | List items must be contained in appropriate list elements |
| `media-captions` | [1.2.2](https://www.w3.org/WAI/WCAG21/Understanding/captions-prerecorded.html) | A | Warning | `<video>` and `<audio>` elements must have `<track>` captions |
| `meta-refresh` | [2.2.1](https://www.w3.org/WAI/WCAG21/Understanding/timing-adjustable.html) | A | Error | `<meta http-equiv="refresh">` must not have a time limit |
| `mouse-events-have-key-events` | [2.1.1](https://www.w3.org/WAI/WCAG21/Understanding/keyboard.html) | A | Error | Mouse event handlers must have corresponding keyboard event handlers |
| `nested-interactive` | [4.1.2](https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html) | A | Error | Interactive elements must not be nested inside other interactive elements |
| `no-access-key` | [2.4.3](https://www.w3.org/WAI/WCAG21/Understanding/focus-order.html) | A | Warning | `accesskey` attribute should not be used |
| `no-autoplay` | [1.4.2](https://www.w3.org/WAI/WCAG21/Understanding/audio-control.html) | A | Warning | `<audio>` and `<video>` must not autoplay without `muted` |
| `no-distracting-elements` | [2.2.2](https://www.w3.org/WAI/WCAG21/Understanding/pause-stop-hide.html) | A | Error | `<blink>` and `<marquee>` elements must not be used |
| `no-duplicate-id` | [4.1.1](https://www.w3.org/WAI/WCAG21/Understanding/parsing.html) | A | Error | `id` attribute values must be unique |
| `no-positive-tabindex` | [2.4.3](https://www.w3.org/WAI/WCAG21/Understanding/focus-order.html) | A | Warning | Avoid `tabindex` values greater than 0 |
| `no-redundant-alt` | [1.1.1](https://www.w3.org/WAI/WCAG21/Understanding/non-text-content.html) | A | Warning | Alt text should not contain words like "image", "picture", "photo" |
| `no-redundant-roles` | [4.1.2](https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html) | A | Warning | Elements should not have redundant ARIA roles |
| `object-alt` | [1.1.1](https://www.w3.org/WAI/WCAG21/Understanding/non-text-content.html) | A | Error | `<object>` elements must have an accessible name |
| `page-title` | [2.4.2](https://www.w3.org/WAI/WCAG21/Understanding/page-titled.html) | A | Error | Document must have a `<title>` element with content |
| `scope-attr` | [1.3.1](https://www.w3.org/WAI/WCAG21/Understanding/info-and-relationships.html) | A | Warning | `scope` attribute should only be used on `<th>` elements |
| `table-header` | [1.3.1](https://www.w3.org/WAI/WCAG21/Understanding/info-and-relationships.html) | A | Warning | `<table>` elements must contain `<th>` header cells |

## WCAG 2.2 Criterion Coverage

### Principle 1: Perceivable

| Criterion | Level | Status |
|-----------|-------|--------|
| 1.1.1 Non-text Content | A | Covered by `img-alt`, `area-alt`, `input-image-alt`, `object-alt`, `no-redundant-alt` |
| 1.2.1 Audio-only and Video-only | A | Not statically checkable (requires content analysis) |
| 1.2.2 Captions (Prerecorded) | A | Covered by `media-captions` |
| 1.2.3 Audio Description or Media Alternative | A | Not statically checkable (requires content analysis) |
| 1.2.4 Captions (Live) | AA | Not statically checkable (requires runtime testing) |
| 1.2.5 Audio Description (Prerecorded) | AA | Not statically checkable (requires content analysis) |
| 1.2.6 Sign Language (Prerecorded) | AAA | Not statically checkable |
| 1.2.7 Extended Audio Description | AAA | Not statically checkable |
| 1.2.8 Media Alternative (Prerecorded) | AAA | Not statically checkable |
| 1.2.9 Audio-only (Live) | AAA | Not statically checkable |
| 1.3.1 Info and Relationships | A | Covered by `form-label`, `heading-order`, `table-header`, `list-structure`, `scope-attr`, `aria-required-children`, `aria-required-parent` |
| 1.3.2 Meaningful Sequence | A | Not statically checkable (requires visual inspection) |
| 1.3.3 Sensory Characteristics | A | Not statically checkable (requires content analysis) |
| 1.3.4 Orientation | AA | Not statically checkable (requires runtime testing) |
| 1.3.5 Identify Input Purpose | AA | Covered by `autocomplete-valid` |
| 1.3.6 Identify Purpose | AAA | Not statically checkable |
| 1.4.1 Use of Color | A | Not statically checkable (requires visual inspection) |
| 1.4.2 Audio Control | A | Covered by `no-autoplay` |
| 1.4.3 Contrast (Minimum) | AA | Not statically checkable (requires computed styles) |
| 1.4.4 Resize Text | AA | Not statically checkable (requires runtime testing) |
| 1.4.5 Images of Text | AA | Not statically checkable (requires content analysis) |
| 1.4.6 Contrast (Enhanced) | AAA | Not statically checkable (requires computed styles) |
| 1.4.7 Low or No Background Audio | AAA | Not statically checkable |
| 1.4.8 Visual Presentation | AAA | Not statically checkable |
| 1.4.9 Images of Text (No Exception) | AAA | Not statically checkable |
| 1.4.10 Reflow | AA | Not statically checkable (requires runtime testing) |
| 1.4.11 Non-text Contrast | AA | Not statically checkable (requires computed styles) |
| 1.4.12 Text Spacing | AA | Not statically checkable (requires runtime testing) |
| 1.4.13 Content on Hover or Focus | AA | Not statically checkable (requires runtime testing) |

### Principle 2: Operable

| Criterion | Level | Status |
|-----------|-------|--------|
| 2.1.1 Keyboard | A | Covered by `click-events-have-key-events`, `mouse-events-have-key-events` |
| 2.1.2 No Keyboard Trap | A | Not statically checkable (requires runtime testing) |
| 2.1.3 Keyboard (No Exception) | AAA | Not statically checkable |
| 2.1.4 Character Key Shortcuts | A | Not statically checkable (requires runtime testing) |
| 2.2.1 Timing Adjustable | A | Covered by `meta-refresh` |
| 2.2.2 Pause, Stop, Hide | A | Covered by `no-distracting-elements` |
| 2.2.3 No Timing | AAA | Not statically checkable |
| 2.2.4 Interruptions | AAA | Not statically checkable |
| 2.2.5 Re-authenticating | AAA | Not statically checkable |
| 2.2.6 Timeouts | AAA | Not statically checkable |
| 2.3.1 Three Flashes or Below | A | Not statically checkable (requires visual analysis) |
| 2.3.2 Three Flashes | AAA | Not statically checkable |
| 2.3.3 Animation from Interactions | AAA | Not statically checkable |
| 2.4.1 Bypass Blocks | A | Covered by `iframe-title` |
| 2.4.2 Page Titled | A | Covered by `page-title` |
| 2.4.3 Focus Order | A | Covered by `no-positive-tabindex`, `no-access-key` |
| 2.4.4 Link Purpose (In Context) | A | Covered by `anchor-content` |
| 2.4.5 Multiple Ways | AA | Not statically checkable (requires site-level analysis) |
| 2.4.6 Headings and Labels | AA | Covered by `heading-content` |
| 2.4.7 Focus Visible | AA | Not statically checkable (requires computed styles) |
| 2.4.8 Location | AAA | Not statically checkable |
| 2.4.9 Link Purpose (Link Only) | AAA | Not statically checkable |
| 2.4.10 Section Headings | AAA | Not statically checkable |
| 2.4.11 Focus Not Obscured (Minimum) | AA | Not statically checkable (requires runtime testing) |
| 2.4.12 Focus Not Obscured (Enhanced) | AAA | Not statically checkable |
| 2.4.13 Focus Appearance | AAA | Not statically checkable |
| 2.5.1 Pointer Gestures | A | Not statically checkable (requires runtime testing) |
| 2.5.2 Pointer Cancellation | A | Not statically checkable (requires runtime testing) |
| 2.5.3 Label in Name | A | Not statically checkable (requires visual analysis) |
| 2.5.4 Motion Actuation | A | Not statically checkable (requires runtime testing) |
| 2.5.5 Target Size (Enhanced) | AAA | Not statically checkable |
| 2.5.6 Concurrent Input Mechanisms | AAA | Not statically checkable |
| 2.5.7 Dragging Movements | AA | Not statically checkable (requires runtime testing) |
| 2.5.8 Target Size (Minimum) | AA | Not statically checkable (requires computed styles) |

### Principle 3: Understandable

| Criterion | Level | Status |
|-----------|-------|--------|
| 3.1.1 Language of Page | A | Covered by `html-lang`, `lang-valid` |
| 3.1.2 Language of Parts | AA | Not statically checkable (requires content analysis) |
| 3.1.3 Unusual Words | AAA | Not statically checkable |
| 3.1.4 Abbreviations | AAA | Not statically checkable |
| 3.1.5 Reading Level | AAA | Not statically checkable |
| 3.1.6 Pronunciation | AAA | Not statically checkable |
| 3.2.1 On Focus | A | Not statically checkable (requires runtime testing) |
| 3.2.2 On Input | A | Not statically checkable (requires runtime testing) |
| 3.2.3 Consistent Navigation | AA | Not statically checkable (requires site-level analysis) |
| 3.2.4 Consistent Identification | AA | Not statically checkable (requires site-level analysis) |
| 3.2.5 Change on Request | AAA | Not statically checkable |
| 3.2.6 Consistent Help | A | Not statically checkable (requires site-level analysis) |
| 3.3.1 Error Identification | A | Not statically checkable (requires runtime testing) |
| 3.3.2 Labels or Instructions | A | Not statically checkable (requires content analysis) |
| 3.3.3 Error Suggestion | AA | Not statically checkable (requires runtime testing) |
| 3.3.4 Error Prevention (Legal, Financial, Data) | AA | Not statically checkable (requires runtime testing) |
| 3.3.5 Help | AAA | Not statically checkable |
| 3.3.6 Error Prevention (All) | AAA | Not statically checkable |
| 3.3.7 Redundant Entry | A | Not statically checkable (requires runtime testing) |
| 3.3.8 Accessible Authentication (Minimum) | AA | Not statically checkable (requires runtime testing) |
| 3.3.9 Accessible Authentication (Enhanced) | AAA | Not statically checkable |

### Principle 4: Robust

| Criterion | Level | Status |
|-----------|-------|--------|
| 4.1.1 Parsing | A | Covered by `no-duplicate-id` |
| 4.1.2 Name, Role, Value | A | Covered by `aria-role`, `aria-props`, `aria-required-attr`, `aria-allowed-attr`, `aria-prohibited-attr`, `aria-valid-attr-value`, `aria-deprecated-role`, `aria-hidden-body`, `aria-hidden-focus`, `nested-interactive`, `button-name`, `no-redundant-roles` |
| 4.1.3 Status Messages | AA | Not statically checkable (requires runtime testing) |
