---
layout: doc
outline: deep
lang: en-US
---

# Important Unsupported Features

Although IronCalc is ready for use, it’s important to understand its current limitations. Below, we list the most significant missing features of a modern spreadsheet engine. If you can live without these features for now, IronCalc might be the product you’re looking for.

## **Collaboration** <Badge type="info" text="Work in progress" />

Real-time collaboration (that is, where multiple users can view and edit the same spreadsheet simultaneously) is not yet available in IronCalc. Currently, spreadsheets cannot be edited concurrently from different devices or by different users. This feature is on the roadmap and is the top priority after the release of version 1.0.

## **Charts** <Badge type="info" text="Work in progress" />

Although charts are an essential feature for any serious spreadsheet program, they are not planned for version 1.0. Adding chart support will become a high priority after the release of version 1.0.


## **Pivot Tables** <Badge type="info" text="Planned" />

Pivot Tables are highly customizable summary tables that let you reorganize and aggregate data in flexible ways. Support for them is planned for a future release.

## **Merged cells through copy & paste** <Badge type="info" text="Planned" />

Merged cells are fully supported by the model API (create, query, and unmerge, and they survive xlsx import/export), but the clipboard does not yet carry merge information. Copying a merged region and pasting it elsewhere pastes only the cell values and styles; it does not reproduce the merge, and pasting over an existing merge does not unmerge it. Carrying merge fidelity through copy & paste is planned for a future release.

::: info
More planned features can be found in our [roadmap](https://www.ironcalc.com/roadmap.html).
:::
