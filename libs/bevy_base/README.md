# Bevy Base (A Rational Re-evaluation of ECS Modularity)

Welcome to a specialized fork of foundational Bevy crates (version 0.19.0). This repository emerged not from a desire to fracture the ecosystem, but from a quiet, unavoidable realization regarding the architectural limits of the official Bevy ECS implementation.

## On the Encapsulation of Identity

In a theoretically sound Entity-Component-System architecture, type identity should be an encapsulated, internal consistency mechanism. Regrettably, in the current Bevy design, the intrinsic Rust `std::any::TypeId` is fundamentally unencapsulated. It bleeds across module boundaries, deeply coupling the framework's semantic logic to ephemeral compiler memory states. 

This architectural oversight becomes most apparent in their treatment of `bevy_reflect`. Instead of mitigating this leakage, the framework allows the `reflect` mechanism to propagate this volatile `TypeId` logic throughout the entire codebase. It is perhaps telling that one cannot even pass the core test suite natively without the `reflect` feature enabled. To prevent this architectural debt from compromising our own systems, we found it strictly necessary to **disable `reflect` entirely**, thereby halting the spread of this coupling at its root.

## The Semantic Misunderstanding of Schedule Labels

A `ScheduleLabel` or `SystemSet` is, at its conceptual core, merely a semantic stub—a token meant for compilation checks and logical grouping. However, the official implementation tightly binds these semantic tags to physical memory addresses via the aforementioned `TypeId`. 

By tying high-level scheduling topology to low-level compiler memory artifacts, the schedule system is structurally stripped of its ability to accept external injection. A system that proudly calls itself a "Plugin architecture" ironically architected itself into a corner where loading an actual dynamic plugin (such as a standard dynamic-link library or shared object) causes immediate runtime collapse due to `TypeId` boundary tearing. 

## Our Humble Course Correction (`StableTypeId`)

This repository is a modest attempt to correct these contradictions. 

We have carefully surgically removed the reliance on Rust's native `TypeId` in the critical interning (`Internable`) and registration paths. In its place, we introduced a `StableTypeId`—a deterministic, purely semantic identifier derived from a stable FNV-1a 64-bit hash of the type's fully qualified path. 

This ensures that a `Component` or a `ScheduleLabel` represents the same mathematical entity regardless of whether it was compiled into the host executable or a hot-swapped DLL plugin.

## For Plugin Authors

This repository must be used as the foundational **ECS Base** for all external plugins within our ecosystem. Distributing this fork ensures that any dynamically loaded libraries will maintain strict semantic and ABI consistency with the host application, fulfilling the true promise of a modular plugin architecture that the original design, perhaps inadvertently, abandoned.

## A Note on Operational Responsibility

Dynamic linking is not a toy abstraction, nor is it a convenience feature for those unwilling to understand the boundaries they are crossing. It is an engineering discipline that demands version control, feature symmetry, ABI awareness, and explicit ownership of failure modes.

This architecture is therefore intended for teams and plugin authors who understand the implications of loading native code across binary boundaries. If these constraints appear excessive, fragile, or unfamiliar, the recommended path is simple: remain within the standard static Cargo dependency model.

Bevy Base does not attempt to make dynamic linking magically safe. It makes the required contracts explicit, deterministic, and enforceable.