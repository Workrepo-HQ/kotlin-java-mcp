package com.example.core;

import lombok.Data;

@Data
public class LombokUser {
    private String username;
    private String email;
    private boolean active;
    private final String id;
}
