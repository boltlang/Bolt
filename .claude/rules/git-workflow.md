# Git & Workflow Rules

- Never commit directly to `main`. Always create a descriptive
  feature/fix branch before making changes.
- *Always test the code before comitting.
- Commit automatically, splitting larger changes in
  multiple smaller commits where this would improve readability.
- When the user approves the commits, use the GitHub CLI
  to open a pull request. Include a clear summary of changes in the PR
  description. The user may type exactly `pr` in the prompt to signify
  approval.

