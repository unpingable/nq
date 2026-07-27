# Synthetic installation operator method

Use fresh model contexts. Supply only:

- one archetype from
  [`installation-operator-scenarios.v1.json`](installation-operator-scenarios.v1.json);
- the scenario's `given` and `task`;
- the ordinary operator documentation in
  [`../INSTALLATION_AND_FIRST_RUN.md`](../INSTALLATION_AND_FIRST_RUN.md);
- the selected source archive or release location;
- an isolated terminal appropriate to the scenario.

Do not provide source code, architecture notes, package dependency graphs,
expected commands, hidden evaluator expectations, or the intended conclusion.
The `hidden_evaluator_expectations` field belongs to scoring and must be
removed from the operator packet.

Use the shared brief:

```text
You are installing an unfamiliar local-first monitoring product called NQ.
Use only the supplied operator documentation, archive or release location,
and terminal. Do not inspect source code or infer missing steps.

Record every command, prompt, permission request, warning, failure, and
recovery decision. Do not repair the environment merely to make installation
pass. State what changed, what remained safe, and which result—if any—was the
first meaningful one.
```

After the run, ask every question in the corpus without showing the expected
answers. Preserve the full prompt, terminal interaction, tool output, and
response verbatim under:

```text
docs/install/campaign/raw/<round>/<scenario>/<model>-<archetype>/
  prompt.md
  transcript.log
  response.md
  result.json
```

Curated scoring belongs outside the raw directory. Never rewrite a failed
command into a cleaner transcript.

At minimum, run:

- two model families where available;
- all five archetypes across the campaign;
- a clean success or bounded-plan case;
- release or dependency refusal;
- unavailable source;
- occupied port;
- older-schema upgrade preflight;
- archive-first reset.

Score recurring confusion as an installation or documentation defect unless
the raw transcript supports scenario ambiguity or evaluator contamination.
Do not use a synthetic result to claim a real non-author trial occurred.
