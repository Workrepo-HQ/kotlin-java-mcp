package com.example.app

import com.example.core.WorkflowConfig
import com.example.core.worksheetWorkflowConfig

class WorkflowUsage {

    fun getConfigs(): List<WorkflowConfig> {
        return listOf(worksheetWorkflowConfig)
    }

    fun printConfig() {
        val config = worksheetWorkflowConfig
        println(config.name)
    }
}
