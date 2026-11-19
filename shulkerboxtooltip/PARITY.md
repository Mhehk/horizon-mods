# ShulkerBoxTooltip — Horizon port parity checklist (mhv2)

Original: ShulkerBoxTooltip 5.4.3+26.3 by MisterPeModder, MIT
(https://github.com/MisterPeModder/ShulkerBoxTooltip, tag `v5.4.3+26.3`, read from
`.build-tools/upstream/ShulkerBoxTooltip`). Port: mihehk. Mod id stays `shulkerboxtooltip`; setting
keys are the Java field paths (`preview.enable`, `controls.previewKey`, ...).

Status: [ ] todo · [x] done · [~] done with a listed difference · [-] not portable (see Differences)

## Config (Configuration.java / ClientConfiguration.java), defaults
### Preview
- [ ] `preview.enable` bool true
- [ ] `preview.swapModes` bool false
- [ ] `preview.alwaysOn` bool false
- [ ] `preview.compactPreviewNbtBehavior` IGNORE | FIRST_ITEM | SEPARATE (default SEPARATE)
- [ ] `preview.compactPreviewOrder` STACK_SIZE | PREVIEW_ORDER (default STACK_SIZE)
- [ ] `preview.defaultMaxRowSize` int 9, validator > 0 ("Must be greater than zero")
- [ ] `preview.serverIntegration` bool true (requires restart)
- [ ] `preview.theme` SHULKERBOXTOOLTIP | VANILLA (default SHULKERBOXTOOLTIP)
- [ ] `preview.themeBundle` VANILLA | VANILLA_PLUS | SHULKERBOXTOOLTIP (default VANILLA)
- [ ] `preview.position` INSIDE | OUTSIDE | OUTSIDE_TOP | OUTSIDE_BOTTOM (default INSIDE)
- [ ] `preview.shortItemCounts` bool true
- [ ] `preview.genericContainerPreview` bool true
### Tooltip
- [ ] `tooltip.showKeyHints` bool true
- [ ] `tooltip.type` VANILLA | MOD | NONE (default MOD)
- [ ] `tooltip.lootTableInfoType` HIDE | SIMPLE | ADVANCED (default HIDE)
- [ ] `tooltip.hideShulkerBoxLore` bool false
### Colors
- [ ] `colors.coloredPreview` bool true
- [ ] colour registry: categories Miscellaneous (default 0xFFFFFF, generic container 0xFFFFFF,
      ender chest 0x0B4B41), Shulker Boxes (plain 0x976797, 16 dyes), Bundles (plain 0x844B2B,
      16 dyes); dye colours = `DyeColor.getTextureDiffuseColor` with each channel ≥ 0.15
### Controls
- [ ] `controls.previewKey` default key.keyboard.left.shift
- [ ] `controls.fullPreviewKey` default key.keyboard.left.alt
- [ ] `controls.lockTooltipKey` default key.keyboard.left.control
### Server (synchronised from a server running the mod)
- [ ] `server.clientIntegration` bool true
- [ ] `server.enderChestSyncType` NONE | ACTIVE | PASSIVE (default ACTIVE)

## Tooltip text (ShulkerBoxTooltipClient.modifyStackTooltip, providers)
- [ ] vanilla container lines (`ItemContainerContents.addToTooltip`) removed unless type == VANILLA
- [ ] lore hidden on shulker boxes when hideShulkerBoxLore
- [ ] appended at the end: provider tooltip when type == MOD: "Contains %s stack(s)" / "Empty" (grey);
      none while the FULL preview shows
- [ ] shulker box with a loot table and lootTableInfoType HIDE: "???????" (grey)
- [ ] loot table SIMPLE: "Has loot table"; ADVANCED: "Loot Table: " + " <id>" (grey)
- [ ] key hints (showKeyHints): "<Preview key>: view contents" or "<Full key>+<Preview key>: view full
      contents" (key gold, text white), swapModes swaps; none when full preview already shows
- [ ] lock hint "<Lock key>: lock tooltip" while a preview shows in a container screen
- [ ] hints only when the provider shows hints (shulker: always; others: when not empty)
- [ ] nothing when the stack's tooltip_display hides the tooltip

## Providers (ShulkerBoxTooltip.registerProviders)
- [ ] shulker boxes (plain + 16 dyes): 27 slots, 9/row, loot tables, colour per dye
- [ ] chest, trapped_chest, 8 copper chests, barrel: 27 slots, 9/row, loot tables
- [ ] furnace, blast_furnace, smoker: 3 slots, 3/row
- [ ] dropper, dispenser: 9 slots, 3/row, loot tables
- [ ] hopper: 5, 5/row, loot · brewing_stand: 5, 5/row · chiseled_bookshelf: 6, 3/row
- [ ] decorated_pot: 1, 1/row, loot · 11 shelves: 3, 3/row
- [ ] lectern: the book
- [ ] ender chest (cache from the player's own ender chest screen; "Open your ender chest to enable
      preview" when empty cache), colour ENDER_CHEST
- [ ] bundles per themeBundle
- [ ] generic container preview for any other item with `minecraft:container` (genericContainerPreview)
- [ ] shouldDisplay: false when loot table & provider uses loot tables, false when no items

## Preview rendering (Mod/Vanilla/BasePreviewRenderer, PreviewClientTooltipComponent)
- [ ] preview type: COMPACT when key held (no full mode available), swapModes logic, alwaysOn
- [ ] compact merging: ItemKey (id ± components), strategies IGNORE/FIRST_ITEM/SEPARATE, comparator
      STACK_SIZE (count desc, then first slot asc) / PREVIEW_ORDER
- [ ] SHULKERBOXTOOLTIP theme: nine-slice `shulker_box_tooltip` sprite (32×32, border 7, tiled inner)
      tinted with the colour, size 14 + cols*18 × 14 + rows*18, slots at +8 every 18
- [ ] inactive slots fill 0x40000000 / excess 0x70000000 (full mode)
- [ ] item + decorations + own count label (abbreviated "1.2k" when shortItemCounts, compact only)
- [ ] slot highlight back/front sprites when hovered/selected
- [ ] VANILLA theme: bundle slot_background / highlight sprites, 24×24 cells, centred, tooltip
      background when outside
- [ ] position INSIDE (height + 4), OUTSIDE (auto top/bottom), OUTSIDE_TOP, OUTSIDE_BOTTOM, side
      fallback when off-screen (24 px gap)
- [ ] inner tooltip of the hovered preview slot (lock mode)

## Controls
- [ ] keys read on every key event (KeyboardHandler.keyPress) as raw key state
- [ ] lock tooltip: holding the lock key freezes the tooltip on its slot and position; the mouse can
      move over the preview to see inner item tooltips

## Config screen (impl/config/gui)
- [ ] title "ShulkerBoxTooltip Configuration", tabs Preview / Tooltip / Colors / Controls / Server
- [ ] entries: boolean, enum (cycle), integer (text field + validator), colour (hex + swatch), key
- [ ] per entry: tooltip text, "R" reset to default, "U" undo change
- [ ] "Save & Quit", "Discard Changes" / quit confirm screen, restart-required screen
- [ ] server category prefix text
- [ ] opened from Options > Mods

## Networking (server integration)
- [ ] handshake + ender chest sync with a server running ShulkerBoxTooltip

## Differences (filled in as the port proceeds)
