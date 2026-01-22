---
name: Pull Request
description: Submit changes to the repository
body:
  - type: markdown
    attributes:
      value: |
        ## Pull Request Checklist
        - [ ] I have read the [contributing guidelines](CONTRIBUTING.md)
        - [ ] My code follows the coding standards
        - [ ] Tests have been added/updated
        - [ ] Documentation has been updated
  - type: input
    id: pr_number
    attributes:
      label: Related Issue Number
      description: "Closes #123" or "Fixes #456"
      placeholder: "#123"
  - type: textarea
    id: description
    attributes:
      label: Description
      description: Summary of changes
      placeholder: What did you change and why?
    validations:
      required: true
  - type: dropdown
    id: type
    attributes:
      label: Change Type
      options:
        - Bug Fix
        - New Feature
        - Breaking Change
        - Refactoring
        - Performance Improvement
        - Testing
        - Documentation
        - Build/CI
        - Other
    validations:
      required: true
  - type: textarea
    id: testing
    attributes:
      label: Testing
      description: Describe the testing performed.
      placeholder: |
        - Unit tests added: X
        - Integration tests added: Y
        - Manual testing: Z
  - type: textarea
    id: checklist
    attributes:
      label: Checklist
      value: |
        - [ ] Code follows project style guidelines
        - [ ] Self-review completed
        - [ ] Comments added for complex logic
        - [ ] Tests pass locally
        - [ ] Benchmarks show no regression (if applicable)
        - [ ] Documentation updated
        - [ ] No new clippy warnings
        - [ ] No new compiler warnings
