# Final redesign v2 raw evidence

These artifacts are raw synthetic fixture evidence. They are not live-deployment
evidence and they are not real-operator trials.

The canonical screenshots for campaign conclusions are:

- `overview/page-stale-label-corrected.png`
- `error-shift/page.png`
- `error-shift/action-preview-suppress.png`
- `stale/page.png`
- `missing/page.png`
- `conflict/page.png`
- `historical/page.png`
- `self-health/page.png`

`overview/page.png` is retained as an intermediate capture. The fresh operator
received `page-stale-label-corrected.png`, which captures the final
implementation after the in-page freshness-label correction. Earlier `redesign/`,
`final-redesign/`, and `post-redesign/` directories remain unchanged as raw
iteration evidence; they must not be substituted for this directory when
claiming final UI behavior.

Fresh operator transcripts were produced from screenshots only, without source
code, architecture notes, intended answers, or evaluator assistance. They are
preserved verbatim beneath the relevant scenario directory. Model-family
coverage is OpenAI only; a second-family run was not completed, so the
campaign does not claim the required two-family acceptance threshold.
