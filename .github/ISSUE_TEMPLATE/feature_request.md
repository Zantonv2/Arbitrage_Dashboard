---
name: Feature Request
description: Suggest a new feature or improvement
labels: enhancement
body:
  - type: markdown
    attributes:
      value: |
        ## Feature Request
        Thank you for suggesting a new feature! Please fill out the form below.
        
        ## Before Submitting
        - [ ] I searched existing issues to avoid duplicates
        - [ ] I checked the roadmap for similar plans
  - type: textarea
    id: feature_summary
    attributes:
      label: Feature Summary
      description: A clear and concise summary of the proposed feature.
      placeholder: "I want to be able to..."
    validations:
      required: true
  - type: textarea
    id: problem
    attributes:
      label: Problem Statement
      description: |
        What problem does this solve?
        Why is this feature needed?
    validations:
      required: true
  - type: textarea
    id: solution
    attributes:
      label: Proposed Solution
      description: Describe your proposed solution.
      placeholder: "The feature should..."
  - type: textarea
    id: alternatives
    attributes:
      label: Alternatives Considered
      description: What other approaches have you considered?
  - type: textarea
    id: context
    attributes:
      label: Additional Context
      description: Add any other context, screenshots, or examples.
