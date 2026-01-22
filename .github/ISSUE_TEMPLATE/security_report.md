---
name: Security Vulnerability
description: Report a security vulnerability
labels: security
body:
  - type: markdown
    attributes:
      value: |
        ## Security Vulnerability Report
        Thank you for reporting a security issue! Please be careful not to disclose this publicly.
        
        ## Responsible Disclosure
        - Do NOT open a public issue for security vulnerabilities
        - Email: security@example.com
        - We will respond within 24 hours
  - type: textarea
    id: vulnerability
    attributes:
      label: Vulnerability Description
      description: Description of the vulnerability.
      placeholder: Describe the vulnerability...
    validations:
      required: true
  - type: textarea
    id: impact
    attributes:
      label: Impact
      description: What is the potential impact if exploited?
    validations:
      required: true
  - type: textarea
    id: affected
    attributes:
      label: Affected Components
      description: Which parts of the system are affected?
  - type: textarea
    id: reproduction
    attributes:
      label: Reproduction Steps
      description: How to reproduce the vulnerability.
  - type: input
    id: severity
    attributes:
      label: Severity (Optional)
      description: Estimated severity (Critical, High, Medium, Low)
