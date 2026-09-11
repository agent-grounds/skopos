# Research rationale

The initial review supports a profiling experiment, with utility evaluated before
service expansion. It does not establish that collecting a graph optimizes
project memory. This document records external precedents, not normative
dependencies on their current behavior. [§GOAL-utility](goals.md#goal-utility-useful-decisions-before-a-larger-service)

| Precedent | Lesson for the prototype |
| --- | --- |
| [Agent-Blackbox](https://github.com/TaewoooPark/Agent-Blackbox/blob/main/docs/analysis.md) | Read capture, context-efficiency heuristics, and instruction suggestions already exist. A generic recorder is insufficient differentiation. |
| [NavTracks](https://plg.uwaterloo.ca/~migod/846/papers/icsm05-navtracks.pdf) | Navigation-derived file associations and related-file recommendations predate coding agents. |
| [Code Maat](https://github.com/adamtornhill/code-maat) | Git co-change is an inexpensive baseline for empirical relationships. |
| [Aider repository maps](https://aider.chat/2023/10/22/repomap.html) and [Serena](https://github.com/oraios/serena) | Static ranking and symbol retrieval can already reduce context acquisition. |
| [Gryph](https://github.com/safedep/gryph), [AgentSight](https://github.com/eunomia-bpf/agentsight), [Confessor](https://github.com/ninjahawk/Confessor), [OpenLIT](https://github.com/openlit/openlit) | Capture infrastructure has substantial prior art; preserve the distinction between system access, tool return, and model submission. |
| [GEPA](https://arxiv.org/abs/2507.19457) and [ACE](https://arxiv.org/abs/2510.04618) | Execution feedback and evaluated interventions are the missing optimization loop. |
| [Evaluating AGENTS.md](https://arxiv.org/abs/2602.11988) | Static context files did not consistently improve success in the evaluated settings and increased cost; this does not directly evaluate Grund section retrieval. |
| [The Working Set of a Coding Agent](https://arxiv.org/abs/2608.16630) | Read histories do not establish which facts were necessary or used; validate produced work. |

The intended differentiation is the join between context observations, historical
Grund sections, Rhei task/cost records, and controlled changes to project memory.
Only the observation and descriptive-analysis parts are implemented initially.
[§GRUND-skopos](grund.md#grund-skopos-project-memory-needs-empirical-feedback) [§RM-validation](roadmap.md#rm-validation-earn-the-next-prototype-stage)

The Pi adapter follows the upstream
[v3 session format](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/session-format.md).
It is intentionally limited to one selected tree ancestry and paired plain-text
read results. The synthetic fixtures exercise those contracts; compatibility
with other native versions is not assumed. [§FS-pi](../requirements.md#fs-pi-import-a-frozen-pi-session-branch)
