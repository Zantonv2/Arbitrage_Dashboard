---
name: Bug Report
description: Report a bug in the arbitrage trading system
labels: bug
body:
  - type: markdown
    attributes:
      value: |
        ## Bug Report
        Thank you for reporting a bug! Please fill out the form below to help us fix it.
        
        ## Before Submitting
        - [ ] I searched existing issues to avoid duplicates
        - [ ] I verified the bug still exists on the latest version
        - [ ] I included steps to reproduce the bug
  - type: textarea
    id: description
    attributes:
      label: Bug Description
      description: A clear and concise description of what the bug is.
      placeholder: Describe the bug here...
    validations:
      required: true
  - type: textarea
    id: steps
    attributes:
      label: Steps to Reproduce
      description: |
        Steps to reproduce the behavior:
        1. Go to '...'
        2. Click on '....'
        3. See error
      placeholder: |
        1. First step
        2. Second step
        3. ...
    validations:
      required: true
  - type: textarea
    id: expected
    attributes:
      label: Expected Behavior
      description: What you expected to happen.
    validations:
      required: true
  - type: textarea
    id: actual
    attributes:
      label: Actual Behavior
      description: What actually happened.
      placeholder: The bug occurred...
    validations:
      required: true
  - type: input
    id: version
    attributes:
      label: Version
      description: Version or commit hash where bug was found
      placeholder: e.g., v1.2.0 or abc1234
  - type: dropdown
    id: component
    attributes:
      label: Component
      description: Which part of the system is affected?
      options:
        - Core Engine
        - Exchange Connector
        - Server (HTTP/WebSocket)
        - Configuration
        - Documentation
        - Other
    validations:
      required: true
  - type: textarea
    id: logs
    attributes:
      label: Relevant Logs
      description: Please include any relevant log output.
      render: shell
  - type: textarea
    id: context
    attributes:
      label: Additional Context
      description: Add any other context about the problem here.
