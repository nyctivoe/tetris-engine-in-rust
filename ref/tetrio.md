# TETR.IO Tetra League — Season 2 Ruleset Spec

## Scope

This document describes **Tetra League Season 2 only**.

It intentionally focuses on the Season 2 ruleset family used for ranked 1v1 play and excludes:

- Season 1 / pre-Season 2 ranked behavior except where needed for comparison
- Quick Play–specific exceptions unless they help explain what **does not** apply to Tetra League
- Custom-room toggles that are not part of the default ranked ruleset
- Character-system proposals or other future overhauls outside the Season 2 ruleset

Where public documentation distinguishes between the **original Season 2 launch/pre-season behavior** and a **later official Season 2 balance update**, both are noted. The last section gives a practical “implement this” summary.

---

## 1. Ranked mode context

Tetra League is TETR.IO’s ranked 1v1 mode. A Season 2 ruleset spec mainly needs to define the following:

- combo behavior
- back-to-back behavior
- surge behavior
- spin recognition
- opener-phase interaction
- default rotation / kick behavior
- default non-ruleset assumptions such as passthrough and clutch behavior

Season 2’s defining change was the replacement of the old default **Back-to-Back Chaining** model with **Back-to-Back Charging / Surge** in ranked play.

---

## 2. Core Season 2 identity

Season 2’s competitive identity is built around four mechanics:

1. **Multiplier combo system** rather than flat combo garbage
2. **Back-to-Back Charging / Surge** rather than classic chaining as the default ranked B2B model
3. **All-Mini / later All-Mini+** spin handling, which makes all-spins relevant to B2B maintenance
4. **Opener Phase**, which changes early cancel behavior for the first 14 pieces

In practice, this means Season 2 rewards:

- preserving B2B more aggressively than earlier Tetra League did
- ending combos on real attacks instead of farming low-value singles
- using all-spins mainly as B2B sustainers rather than as full T-spin replacements
- understanding TETR.IO-specific kicks, especially 180 and SRS+

---

## 3. Default board / move assumptions relevant to the ruleset

These are not unique to Season 2, but they are part of the normal ranked environment Season 2 sits on top of:

- playfield: **10 × 40**
- hold: **enabled**
- hard drop: **enabled**
- default rotation system: **SRS+**
- default input ecosystem includes **180° rotation support**

### 3.1 SRS+

TETR.IO’s default rotation system is **SRS+**, not plain guideline SRS. The important practical difference is that the **I-piece wall kicks are modified to be symmetric**, allowing placements that do not exist in regular SRS.

### 3.2 180 kicks

TETR.IO also has a custom **180 kick table**. In ranked practice, this matters because many Season 2 all-spin continuations and efficient tucks depend on 180 support. If you are implementing or testing a Season 2 engine, vanilla “no 180” handling is not an accurate default.

---

## 4. Combo system: Multiplier

Season 2 Tetra League uses TETR.IO’s **Multiplier** combo system.

### 4.1 Formula

For a clear with positive base attack:

```text
combo_attack = base_attack × (1 + 0.25 × combo)
```

For a clear with zero base attack, TETR.IO uses a logarithmic fallback from 2-combo onward:

```text
combo_attack = ln(1 + 1.25 × combo)
```

### 4.2 Rounding

In multiplayer outside Quick Play, the default rounding mode is **DOWN**. That means ranked Tetra League floors these values instead of using Quick Play’s weighted RNG rounding.

### 4.3 Strategic meaning

This is one of the reasons classic 4-wide style low-base combo spam is weaker in TETR.IO than in many other versus stackers. Season 2’s combo system rewards combos that **terminate in a strong base attack** such as a Quad or T-Spin, rather than long strings of weak clears.

---

## 5. Season 2 back-to-back model: Charging / Surge

### 5.1 What changed from older Tetra League

The old default ranked mechanic was **Back-to-Back Chaining**. In Season 2, the default ranked mechanic became **Back-to-Back Charging** instead.

This is the single most important ruleset change of Season 2.

### 5.2 What counts as a difficult clear

Under the public Season 2 descriptions, B2B is maintained by doing consecutive **difficult line clears** without interrupting them with a normal Single, Double, or Triple.

The public documentation describes difficult clears as including:

- **Quads**
- **T-Spins**
- **All-Spins when enabled under the active spin ruleset**

### 5.3 What the system does

Charging / Surge has two simultaneous effects:

1. difficult clears continue your B2B state and add a B2B attack bonus
2. repeated B2Bs also **bank Surge**, which is released when you break the streak

When the streak breaks, all stored Surge lines are sent at once.

### 5.4 Surge segmentation

Stored Surge is not sent as one indivisible packet. Public documentation says it is split into **three segments**, with the remainder carried by the first segment and sometimes the second.

That means a large broken B2B chain becomes a multi-part spike rather than one monolithic block.

---

## 6. Original Season 2 launch behavior vs later Season 2 rebalance

Public sources show two relevant Season 2 states:

### 6.1 Original Season 2 / pre-season direction

The original Season 2 patch-note summary describes the new ranked identity like this:

- all spins now count as B2B
- repeated B2Bs build charge that releases at the end of the chain

Public mechanical summaries for the early Charging system describe Surge in non-Quick-Play multiplayer as follows:

- Surge begins at **B2B streak 4**
- in non-Quick-Play multiplayer it starts at **4 stored lines**
- each additional B2B adds more stored Surge

This is the cleanest way to think about the **initial** Season 2 design.

### 6.2 Later official Tetra League balance update within Season 2

A later official TETR.IO patch-note snippet describes a direct **Tetra League balance** update that still belongs to the Season 2 ruleset family:

- **Back-to-Back required to start a Surge decreased to 3 (was 4)**
- **Upon reaching B2Bx3, start with 3 Surge instead of 1**
- **All Clears now send 5 garbage (was 3)**
- **All Clears now count as normal Back-to-Back** instead of using the old “+2 B2B” handling

### 6.3 Practical conclusion

If you want the **latest playable Season 2 standard**, use the later official balance interpretation:

- Surge starts at **B2Bx3** rather than B2Bx4
- the first active Surge state begins with **3 stored lines**
- All Clears behave like a normal high-value B2B attack and send **5**

If you want the **historical original Season 2 launch behavior**, use the earlier B2Bx4 start model.

---

## 7. B2B bonus ladder

Season 2 does not merely care about “B2B on or off.” Public TETR.IO documentation and public community calculators indicate that B2B attack bonus increases by levels as the displayed B2B count grows.

A practical Season 2 ladder is:

| Displayed B2B count | Extra attack bonus |
|---|---:|
| active B2B up to **B2Bx2** | +1 |
| **B2Bx3** to **B2Bx7** | +2 |
| **B2Bx8** to **B2Bx23** | +3 |
| **B2Bx24** to **B2Bx66** | +4 |
| **B2Bx67** to **B2Bx184** | +5 |
| **B2Bx185** to **B2Bx503** | +6 |
| **B2Bx504** to **B2Bx1369** | +7 |
| **B2Bx1370** and above | +8 |

This matters because Season 2 rewards both:

- preserving the chain to keep the per-attack B2B bonus high
- deciding **when** to cash out the stored Surge

---

## 8. Spin rules in Season 2

### 8.1 All-Mini → All-Mini+

When Beta began, TETR.IO introduced **All-Mini**, where non-T pieces could perform spins using **immobile detection** and those spins were treated as **Mini-Spins**.

Later, **All-Mini+** replaced All-Mini as the default multiplayer rule. All-Mini+ extends the same philosophy to T-pieces as well, allowing immobile detection there too.

For Season 2 as currently documented, **All-Mini+** is the correct default ranked assumption.

### 8.2 What all-spins mean in practice

Under Season 2 public descriptions:

- all-spins are relevant to B2B
- all-spins are generally treated as **Mini-like** sustain tools in the default ruleset
- they are valuable mainly because they **preserve B2B / feed Surge**, not because they replace full T-Spin Double or Triple damage

This is why high-level Season 2 play often treats non-T spins as “glue” between larger attacks.

### 8.3 T-piece mini handling under All-Mini+

A later official patch-note snippet states that **immobile T-Spins that do not fulfill the three-corner rule count as Minis**. That is an important Season 2 detail: under the later default rules, T-piece detection is more permissive than a strict classic-only corner interpretation.

### 8.4 Non-clear spins

Season 2 sources also show that non-clear spins became mechanically relevant in some Season 2 contexts, because spins are tied into B2B state rather than being purely cosmetic. If you are implementing a Season 2 engine, do not assume “spin without line clear does nothing” in every case unless you are modeling the exact current mode-specific exceptions.

---

## 9. Kicks and rotation behavior that matter for Season 2

### 9.1 Default competitive assumption

The default ranked assumption is:

- **SRS+**
- **TETR.IO 180 kicks enabled**

### 9.2 Why this matters strategically

Season 2 all-spin sustain routes, many rescue tucks, and a large number of efficient continuations depend on TETR.IO’s kick behavior. In particular:

- SRS+ changes what the I-piece can reach
- custom 180 kicks create additional L/J/S/Z/I tuck possibilities
- some all-spin sustain routes are practical only with 180 support

For engine testing, this means “guideline SRS plus no 180” is not a faithful Season 2 ruleset.

---

## 10. Opener Phase

Season 2 uses **Opener Phase** by default in multiplayer, including Tetra League.

### 10.1 Rule

For the first **14 pieces placed**, if the attack you send that placement is **less than the amount of garbage pending against you**, you cancel **twice as much**.

### 10.2 Practical effect

This makes the early game meaningfully different from neutral midgame. Openers are not judged only by raw spike size; they are also shaped by whether they:

- preserve B2B
- convert well into early Surge
- interact efficiently with double cancel during the opener window

---

## 11. All Clears in Season 2

There are two relevant Season 2 public states:

### 11.1 Earlier / non-current Season 2 handling

Older public summaries describe non-Quick-Play multiplayer All Clears differently from what later official Tetra League balance uses.

### 11.2 Later Season 2 Tetra League handling

The later official Tetra League balance note is the clearer current standard:

- **All Clears send 5 garbage**
- **All Clears count as normal B2B**

So for a modern Season 2 Tetra League implementation, treat All Clear as a normal B2B-qualifying attack worth **5** sent lines.

---

## 12. Passthrough and garbage timing

### 12.1 Passthrough

Passthrough is **not part of the standard Season 2 Tetra League default**. Public documentation states passthrough is disabled by default in all gamemodes and can only be re-enabled in customs.

### 12.2 Garbage travel / queue timing

Public documentation notes a travel time of **20 frames / 0.333 seconds** for passthrough-era garbage interactions, but since passthrough is not part of the default ranked ruleset, it should not be treated as a defining Season 2 ranked mechanic.

If your goal is a ranked-faithful engine, the safer interpretation is:

- no default passthrough
- normal pending-garbage / cancel behavior
- opener phase modifies cancel efficiency early on

---

## 13. Clutch Clear behavior relevant to Season 2

After Beta 1.5.0, public documentation says Clutch Clear was reworked so that if you clear lines in a situation that would have caused a **block out**, your next piece can be pushed above the stack and the game continues.

This is not the headline mechanic of Season 2, but it is part of the modern ranked environment the Season 2 ruleset lives in.

---

## 14. Recommended implementation spec for a Season 2 engine

If the goal is to implement **Season 2 Tetra League as currently understood from public docs**, use the following rules:

### 14.1 Environment

- board: 10×40
- hold: on
- hard drop: on
- default rotation: SRS+
- 180 rotation / kicks: on
- passthrough: off by default

### 14.2 Combo

- combo model: Multiplier
- formula: `base * (1 + 0.25 * combo)`
- if base attack is 0 and combo ≥ 2: `ln(1 + 1.25 * combo)`
- rounding in ranked: floor / DOWN

### 14.3 B2B / Surge

- difficult clears: Quads, T-Spins, and valid all-spins under the active spin rules
- normal Single / Double / Triple breaks the B2B chain
- breaking the chain releases all stored Surge
- released Surge splits into 3 segments
- use the **later Season 2 Tetra League balance** as the default live spec:
  - Surge starts at **B2Bx3**
  - reaching **B2Bx3** starts with **3 stored Surge**
  - All Clear sends **5** and qualifies as normal B2B

### 14.4 Spin rules

- default spin ruleset: **All-Mini+**
- non-T spins use immobile detection and count as mini-type spins in the default ruleset
- T-piece immobile cases that do not satisfy the stricter corner condition count as Minis under the later patch-note wording
- all-spins preserve B2B and therefore matter for Surge routing

### 14.5 Opener Phase

- for first **14 placed pieces**:
  - if outgoing attack `<` pending incoming,
  - cancel at **2× efficiency**

---

## 15. Original-launch variant if you want historical Season 2 instead of latest Season 2

If you specifically want the **historical original Season 2 launch flavor** rather than the latest official Season 2 balance state, change only these points:

- Surge begins at **B2B4** instead of B2Bx3
- non-Quick-Play multiplayer descriptions treat the first active Surge state as starting from the older non-current value set
- do **not** use the later “All Clear = 5 and normal B2B” rebalance unless you are explicitly targeting the later Season 2 patch state

Everything else in this document can stay broadly the same.

---

## 16. Short summary

Season 2 Tetra League is best understood as:

- **Multiplier combo** ranked garbage math
- **Charging / Surge** instead of old default chaining
- **All-Mini+** spin logic, making all-spins B2B sustain tools
- **SRS+ with 180 kicks** as the default movement environment
- **Opener Phase** for the first 14 pieces
- **latest live Season 2 balance**: Surge starts earlier at **B2Bx3**, and **All Clears send 5 and behave like normal B2B**

That combination is the distinctive Season 2 identity.

---

## References

1. TETR.IO patch notes (official): https://tetr.io/about/patchnotes/
2. TetrisWiki — TETR.IO: https://tetris.wiki/TETR.IO
3. TETRIO Statistics FAQ / public attack references: https://tetrio.team2xh.net/faq
4. TETRIO Statistics main site: https://www.tetrio.team2xh.net/
