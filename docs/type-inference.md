# Type Inference Limitations

This tool uses tree-sitter for syntactic analysis only — it does not have a type inference engine. This means symbol resolution relies on import analysis and name matching rather than resolved types.

## What works well

- Resolving symbols through explicit imports, wildcard imports, and same-package declarations
- Cross-language references (Kotlin ↔ Java) when imports are present
- Lombok field usages in files that import the containing class

## Where it falls short — member access without import context

When code accesses a member through a chain like `ctx.getService().getConfig().fieldName`, resolving `fieldName` to the correct class requires knowing the return type of each method in the chain. This is type inference, which has escalating levels of complexity:

1. **Local variable type tracking** — parse `val x: Foo = ...` or `Foo x = ...` to know `x` is `Foo`, then resolve `x.field` → `Foo.field`. Moderate effort (~300-400 lines), but only handles the simple single-hop case.

2. **Method return type tracking** — index return types on method declarations so `x.getConfig()` can be resolved if `getConfig()` has a declared return type. Significant effort (~500+ lines on top of level 1). Generics (`<T> T getParam(Class<T>)`) make this dramatically harder since it requires generic type substitution.

3. **Full type inference** — lambda receivers (Kotlin's `apply { this is X }`), smart casts, generic resolution, overload resolution. This is building a compiler frontend — thousands of lines and months of work. At that point, embedding the Kotlin/Java compiler APIs directly would be more practical.

## Current approach

Import-based filtering is used as a proxy for type information: if a file doesn't import class `Foo`, references to `fieldName` in that file are unlikely to be `Foo.fieldName`. This eliminates most false positives without requiring type inference.
