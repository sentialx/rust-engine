# Merge Analysis: Master → Refactors Branch

## Summary
Comparing changes on `master` since commit `f498e16367c7638300bf8aa340db0be885a1ce20` to determine what can be merged into the `refactors` branch.

## Commits on Master (since f498e163)
1. `30644d3` - update deps
2. `2e585fa` - refactor
3. `1e5b7f7` - input, border, event handling

## Key Differences

### Architecture Differences
- **Master**: Uses `css_properties.rs` with `f64` types, simpler CSS property computation
- **Refactors**: Uses `properties/` directory with `CssValue` types, `f32` types, more modular property system
- **Note**: You've already updated `colors.rs` to use `f32` instead of `f64`, which aligns with refactors branch

## Mergeable Features from Master

### ✅ Easy to Merge (New Files)
1. **`src/input_element.rs`** (490 lines)
   - New file with `CustomRenderer` and `EventHandler` traits
   - Input element handling (caret, focus, keyboard events)
   - Should merge cleanly as it's a new file
   - **Action**: Cherry-pick this file

2. **`default_styles.css`** (16 lines added)
   - Default CSS styles for HTML elements
   - Includes input element styles with border support
   - **Action**: Merge this file

3. **`ui_demo.html`** (328 lines)
   - Demo HTML file for testing
   - **Action**: Merge this file

### ⚠️ Requires Adaptation (Modified Files)

1. **Border Support**
   - Master adds `border: Border` field to `ComputedStyle`
   - Master adds border shorthand parsing in `css.rs`
   - Master adds border rendering in `layout.rs`
   - **Challenge**: Refactors branch has different `ComputedStyle` structure (in `styles.rs`)
   - **Action**: 
     - Add `Border` struct to refactors branch (adapt from master's `css_properties.rs`)
     - Add border field to `ComputedStyle` in `styles.rs`
     - Add border shorthand parsing to refactors' `css.rs`
     - Add border rendering to `layout.rs`

2. **Event Handling Refactor**
   - Master refactors hover handling to use `EventHandler` trait
   - Master adds `handle_mouse_move` to `EventHandler`
   - **Challenge**: Need to adapt to refactors branch structure
   - **Action**: 
     - The `input_element.rs` already has the event handling
     - Need to integrate `handle_mouse_move` into refactors' `browser_window.rs`

3. **Browser Window Changes**
   - Master has significant changes to `browser_window.rs` for:
     - Input element support
     - Event routing via `EventHandler` trait
     - Hover event handling refactor
   - **Challenge**: Refactors branch likely has different structure
   - **Action**: Review and manually merge relevant sections

4. **Layout Changes**
   - Master adds border rendering logic
   - Master refactors hover handling
   - **Challenge**: Refactors uses `f32` vs master's `f64` (but you've already started fixing this)
   - **Action**: Review and adapt border rendering, ensure type consistency

5. **HTML/CSS Changes**
   - Master adds border field to `ComputedStyle`
   - Master adds border shorthand parsing
   - **Action**: Adapt to refactors' structure

### ❌ Likely Conflicts

1. **Type System**
   - Master uses `f64`, refactors uses `f32`
   - You've already started fixing this in `colors.rs`
   - **Action**: Continue converting `f64` → `f32` throughout

2. **CSS Property System**
   - Master: `css_properties.rs` with simple computation
   - Refactors: `properties/` directory with `CssValue` system
   - **Action**: Either:
     - Add border support to refactors' property system, OR
     - Adapt master's border code to work with refactors' system

3. **CSS Parsing**
   - Different structures between branches
   - **Action**: Manually merge border shorthand parsing

## Recommended Merge Strategy

### Phase 1: Easy Wins
1. Cherry-pick `input_element.rs` from master
2. Merge `default_styles.css`
3. Merge `ui_demo.html`

### Phase 2: Border Support
1. Create `Border` struct compatible with refactors' type system (`f32`)
2. Add border field to `ComputedStyle` in `styles.rs`
3. Add border shorthand parsing to refactors' `css.rs`
4. Add border rendering to `layout.rs`

### Phase 3: Event Handling Integration
1. Integrate `handle_mouse_move` from `input_element.rs` into refactors' event system
2. Update `browser_window.rs` to use event handler routing
3. Test hover functionality

### Phase 4: Type Consistency
1. Complete `f64` → `f32` conversion throughout codebase
2. Ensure all new code uses `f32`

## Files to Review

### From Master (need to merge):
- `src/input_element.rs` ✅ (new file, should be easy)
- `default_styles.css` ✅ (new file, should be easy)
- `ui_demo.html` ✅ (new file, should be easy)
- `src/css_properties.rs` ⚠️ (adapt border struct)
- `src/browser_window.rs` ⚠️ (review and merge selectively)
- `src/layout.rs` ⚠️ (review border rendering)
- `src/html.rs` ⚠️ (review border field addition)
- `src/css.rs` ⚠️ (review border shorthand parsing)

### Already Deleted on Refactors:
- `devtools.html` (you deleted this, so skip it)

## Next Steps

1. Start with Phase 1 (easy wins)
2. Then tackle border support (Phase 2)
3. Integrate event handling (Phase 3)
4. Fix type consistency (Phase 4)

Would you like me to start with Phase 1 and cherry-pick the easy files?
