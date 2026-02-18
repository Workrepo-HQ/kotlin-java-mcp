package com.example.core

data class WorkflowConfig(
    val name: String,
    val steps: List<String>
)

val worksheetWorkflowConfig = WorkflowConfig(
    name = "worksheet",
    steps = listOf("create", "review", "approve")
)
