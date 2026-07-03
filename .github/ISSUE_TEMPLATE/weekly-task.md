---
name: Weekly task
title: ""
description: Track a project task for the current milestone.
labels: []
body:
  - type: textarea
    id: summary
    attributes:
      label: Summary
      description: Describe the task and its goal.
      placeholder: What needs to be done?
    validations:
      required: true
  - type: textarea
    id: acceptance
    attributes:
      label: Acceptance criteria
      description: Describe what should be true when the task is complete.
      placeholder: Define completion.
    validations:
      required: true
