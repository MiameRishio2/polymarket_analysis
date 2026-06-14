## Fix Solution

Use a fixed SVG coordinate width for `combinedSeriesChart` instead of scaling width by snapshot count. Keep the existing x-position interpolation across all snapshots and the existing `tickStep` sampling, so long histories still show all points but do not create an unbounded horizontal axis.

The CSS should let the chart fill the available timeline container width without forcing a very large min-width. Tests should assert that rendering many snapshots does not emit an oversized inline SVG width.
