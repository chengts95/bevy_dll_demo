//! A domain-agnostic ECS prefab system.
//!
//! Define reusable entity templates in JSON, instantiate them into any target
//! world. Circuits are just one domain; nets/nodes are a domain concern, kept
//! out of the framework core. See `DESIGN.md` for the full design.
//!
//! Modules (one responsibility each):
//! - [`format`]      — on-disk schema (pure data + serde)
//! - [`registry`]    — type registry, serde verbs, flatten/rebuild
//! - [`subst`]       — `$`/`@` param substitution at the Value level
//! - [`instantiate`] — load path: class → world (recursive `Use` expansion)
//! - [`extract`]     — reverse: world → class (auto-detect boundary ports)
//! - [`bind`]        — entity-reference resolution (`$ref:`/`@ref:` → `Entity`)
//! - [`patch`]       — the patch verb (mutate an existing named entity)

mod bind;
mod extract;
mod format;
mod instantiate;
mod patch;
mod registry;
mod subst;

pub use format::Local;
pub use format::*; // disambiguate vs `bevy_ecs::prelude::Local`

pub use bind::{Bound, NameTag, PrefabEntityId, Refs, resolve_refs};
pub use instantiate::NodeAlloc;
pub use patch::Patch;
pub use registry::{ParamOverride, ParameterMapping, PrefabRegistry, PrefabRoot, collect_subtree};
pub use subst::substitute;

/// Fallibly register a batch of component types into a [`PrefabRegistry`].
///
/// Intended for plugin setup code:
/// ```rust,ignore
/// prefab::try_register_components!(reg, Resistor, Capacitor, PortNames)?;
/// ```
#[macro_export]
macro_rules! try_register_components {
    ($registry:expr, $($ty:ty),+ $(,)?) => {{
        $( $registry.try_register::<$ty>()?; )+
        Ok::<(), String>(())
    }};
}

/// Fallibly mark a batch of component types as net-name carriers.
#[macro_export]
macro_rules! try_register_net_components {
    ($registry:expr, $($ty:ty),+ $(,)?) => {{
        $( $registry.try_register_net::<$ty>()?; )+
        Ok::<(), String>(())
    }};
}

// ───────────────────────── unit tests ─────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::Local; // disambiguate vs the bevy_ecs prelude glob below
    use super::*;
    use bevy_ecs::hierarchy::{ChildOf, Children};
    use bevy_ecs::prelude::*;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    // ── feature: prefab deserialization path (format.rs) ────────────────────
    #[test]
    fn deserializes_canonical_library() {
        let lib: Library = serde_json::from_str(
            r#"{
            "vsource": {
                "params": {"v": 10.0},
                "body": [{"id":0,"components":{"Vsource":{"volts":"$v"},"Net":["p","gnd"]}}]
            },
            "R3_grounded": {
                "params": {"r": 1.0},
                "body": [{"id":0,"components":{"Use":{"prefab":"R3","params":{"r":"$r","a1":"a","a2":"gnd"}}}}]
            }
        }"#,
        )
        .unwrap();

        assert_eq!(lib["vsource"].params["v"], serde_json::json!(10.0));
        assert_eq!(lib["vsource"].body.len(), 1);
        let comps = &lib["vsource"].body[0].components;
        assert_eq!(comps["Vsource"]["volts"], serde_json::json!("$v"));
        assert!(lib["R3_grounded"].body[0].components.contains_key("Use"));
    }

    #[test]
    fn use_rejects_removed_import_channel() {
        let value = serde_json::json!({
            "prefab": "leaf",
            "params": {"a": "bus"},
            "import": {"a": "other"}
        });
        assert!(serde_json::from_value::<Use>(value).is_err());
    }

    #[derive(Component, Clone, PartialEq, Debug, Serialize, Deserialize)]
    struct Wheel {
        radius: f32,
    }
    #[derive(Component, Clone, PartialEq, Debug, Serialize, Deserialize)]
    struct Resistor {
        ohms: f32,
    }

    fn reg() -> PrefabRegistry {
        let mut r = PrefabRegistry::default();
        r.register::<Local>()
            .register::<Net>()
            .register::<Wheel>()
            .register::<Resistor>();
        r
    }

    fn build_car(w: &mut World) -> Entity {
        let root = w.spawn(Local("car".into())).id();
        for k in ["fl", "fr", "rl", "rr"] {
            w.spawn((
                Wheel { radius: 0.34 },
                Local(format!("wheel_{k}")),
                Net(vec![format!("mount_{k}"), "chassis".into()]),
                ChildOf(root),
            ));
        }
        root
    }

    #[test]
    fn flatten_rebuild_roundtrip_preserves_tree_and_data() {
        let r = reg();
        let mut a = World::new();
        let root = build_car(&mut a);

        let snap = r.flatten(&a);
        let mut b = World::new();
        let map = r.rebuild(&snap, &mut b);

        let nroot = map[&root.index_u32()];
        assert_eq!(b.get::<Children>(nroot).unwrap().len(), 4);
        let wheels = b
            .iter_entities()
            .filter(|e| e.get::<Wheel>().is_some())
            .count();
        assert_eq!(wheels, 4);
    }

    #[test]
    fn file_is_serde_value_and_format_agnostic() {
        let r = reg();
        let mut a = World::new();
        build_car(&mut a);
        let snap = r.flatten(&a);

        let json = serde_json::to_string(&snap).unwrap();
        let back: Snapshot = serde_json::from_str(&json).unwrap();
        let mut b = World::new();
        r.rebuild(&back, &mut b);
        let wheels = b
            .iter_entities()
            .filter(|e| e.get::<Wheel>().is_some())
            .count();
        assert_eq!(wheels, 4);
        assert!(!json.contains("generation"));
    }

    #[test]
    fn subtree_traversal_is_encapsulated() {
        let mut a = World::new();
        let root = build_car(&mut a);
        assert_eq!(collect_subtree(&a, root).len(), 5);
    }

    #[test]
    fn param_substitution_is_generic_and_childof_is_the_only_hierarchy() {
        #[derive(Component, Clone, PartialEq, Debug, Serialize, Deserialize)]
        struct Resistor(f32);

        let mut reg = PrefabRegistry::default();
        reg.register::<Local>().register::<Resistor>();

        let template: Snapshot = vec![
            EntityRecord {
                id: 0,
                parent: None,
                components: serde_json::Map::from_iter([(
                    "Local".into(),
                    serde_json::json!("bank"),
                )]),
            },
            EntityRecord {
                id: 1,
                parent: Some(0),
                components: serde_json::Map::from_iter([
                    ("Local".into(), serde_json::json!("r_a")),
                    ("Resistor".into(), serde_json::json!("$r")),
                ]),
            },
        ];

        let mut t = template.clone();
        let params = serde_json::Map::from_iter([("r".to_string(), serde_json::json!(5.0))]);
        substitute(&mut t, &params, &serde_json::Map::new());

        let mut world = World::new();
        let map = reg.rebuild(&t, &mut world);

        let child = map[&1];
        assert_eq!(world.get::<Resistor>(child).unwrap().0, 5.0);
        assert!(world.get::<ChildOf>(child).is_some());
        assert_eq!(world.get::<Children>(map[&0]).unwrap().len(), 1);
    }

    // ── feature: $ / @ / ref: symbol distinction (subst.rs) ─────────────────
    #[test]
    fn substitute_distinguishes_local_global_and_defers_refs() {
        let mut snap: Snapshot = vec![EntityRecord {
            id: 0,
            parent: None,
            components: serde_json::Map::from_iter([(
                "C".into(),
                serde_json::json!({
                    "val": "$r",
                    "rail": "@gnd",
                    "probe": "$ref:r1",
                    "actor": "@ref:Player1"
                }),
            )]),
        }];
        let params = serde_json::Map::from_iter([("r".to_string(), serde_json::json!(50.0))]);
        let globals = serde_json::Map::from_iter([("gnd".to_string(), serde_json::json!("gnd"))]);

        substitute(&mut snap, &params, &globals);

        let c = &snap[0].components["C"];
        assert_eq!(c["val"], serde_json::json!(50.0));
        assert_eq!(c["rail"], serde_json::json!("gnd"));
        assert_eq!(c["probe"], serde_json::json!("$ref:r1"));
        assert_eq!(c["actor"], serde_json::json!("@ref:Player1"));
    }

    // ── feature: load path + @gnd global ground (instantiate.rs) ────────────
    #[test]
    fn at_gnd_resolves_to_global_ground() {
        #[derive(Component, Clone, PartialEq, Debug, Serialize, Deserialize)]
        struct Resistor(f32);

        let mut reg = PrefabRegistry::default();
        reg.register::<Net>().register::<Resistor>();
        reg.register_global("gnd", serde_json::json!("gnd"));

        let lib: Library = serde_json::from_str(
            r#"{ "shunt": {
                "params": {"r": 1.0, "a1":"a1","b1":"b1","c1":"c1","a2":"a2","b2":"b2","c2":"c2"},
                "body": [{"id":0,"components":{"Resistor":"$r","Net":["a","@gnd"]}}]
            }}"#,
        )
        .unwrap();

        let mut world = World::new();
        let mut alloc = NodeAlloc::new(1);
        let binding = HashMap::from([("a".to_string(), 5u32)]);
        reg.instantiate_class(
            &lib,
            "shunt",
            &serde_json::Map::new(),
            &binding,
            &mut alloc,
            &mut world,
        );

        let (r, pins): (f32, Vec<u32>) = {
            let mut q = world.query::<(&Resistor, &Pins)>();
            let (r, p) = q.iter(&world).next().unwrap();
            (r.0, p.0.clone())
        };
        assert_eq!(r, 1.0);
        assert_eq!(pins, vec![5, 0]);
    }

    // ── feature: recursive Use composition (instantiate.rs) ─────────────────
    #[test]
    fn multi_level_composition_expands_recursively() {
        #[derive(Component, Clone, PartialEq, Debug, Serialize, Deserialize)]
        struct Resistor(f32);

        let mut reg = PrefabRegistry::default();
        reg.register::<Net>()
            .register::<Resistor>()
            .register::<Use>();

        let lib: Library = serde_json::from_str(
            r#"{
            "R3": {
                "params": {"r":1.0,"a1":"a1","b1":"b1","c1":"c1","a2":"a2","b2":"b2","c2":"c2"},
                "body": [
                    {"id":0,"components":{"Resistor":"$r","Net":["$a1","$a2"]}},
                    {"id":1,"components":{"Resistor":"$r","Net":["$b1","$b2"]}},
                    {"id":2,"components":{"Resistor":"$r","Net":["$c1","$c2"]}}
                ]
            },
            "R3_grounded": {
                "params": {"r":1.0,"a":"a","b":"b","c":"c"},
                "body": [
                    {"id":0,"components":{"Use":{"prefab":"R3","params":{"r":"$r","a1":"$a","b1":"$b","c1":"$c","a2":"gnd","b2":"gnd","c2":"gnd"}}}}
                ]
            },
            "feeder": {
                "params": {"x":"x","y":"y","z":"z"},
                "body": [
                    {"id":0,"components":{"Use":{"prefab":"R3_grounded","params":{"r":7.0,"a":"$x","b":"$y","c":"$z"}}}}
                ]
            }
        }"#,
        )
        .unwrap();

        let mut world = World::new();
        let mut alloc = NodeAlloc::new(1);
        let binding = HashMap::from([
            ("x".to_string(), 10u32),
            ("y".to_string(), 11),
            ("z".to_string(), 12),
        ]);
        reg.instantiate_class(
            &lib,
            "feeder",
            &serde_json::Map::new(),
            &binding,
            &mut alloc,
            &mut world,
        );

        let remaining = {
            let mut q = world.query::<&Use>();
            q.iter(&world).count()
        };
        assert_eq!(remaining, 0);

        let rs: Vec<(f32, Vec<u32>)> = {
            let mut q = world.query::<(&Resistor, &Pins)>();
            q.iter(&world).map(|(r, p)| (r.0, p.0.clone())).collect()
        };
        assert_eq!(rs.len(), 3);
        assert!(rs.iter().all(|(v, _)| *v == 7.0));
        assert!(
            rs.iter()
                .all(|(_, pins)| pins.len() == 2 && pins[1] == 0 && (10..=12).contains(&pins[0]))
        );
    }

    #[test]
    fn nested_use_preserves_child_prefab_hierarchy() {
        let mut reg = reg();
        reg.register::<Use>();
        let lib: Library = serde_json::from_value(serde_json::json!({
            "child": {
                "body": [
                    {"id": 0, "components": {"Local": "line"}},
                    {"id": 1, "parent": 0, "components": {"Local": "ia"}}
                ]
            },
            "root": {
                "body": [
                    {"id": 0, "components": {"Use": {"prefab": "child"}}}
                ]
            }
        }))
        .unwrap();
        reg.load_library(&lib).unwrap();
        let mut world = World::new();
        reg.spawn_class(&lib, "root", &serde_json::Map::new(), "", &mut world);

        let mut q = world.query::<(Entity, &Local)>();
        let entities: HashMap<String, Entity> = q
            .iter(&world)
            .map(|(entity, local)| (local.0.clone(), entity))
            .collect();
        let line = entities["line"];
        let ia = entities["ia"];
        assert_eq!(world.get::<ChildOf>(ia).unwrap().parent(), line);
        assert_ne!(world.get::<ChildOf>(line).unwrap().parent(), line);
    }

    // ── feature: spawn_class recursive composition + net qualification ──────
    // Two instances of the same `branch` prefab inside a `pair`: their internal
    // `mid` nets must be qualified distinct, while ports/globals wire through.
    // Net resolution is the domain's job — here we just check the produced names.
    #[test]
    fn spawn_class_recursive_qualifies_internals_and_wires_ports() {
        #[derive(Component, Clone, Debug, Serialize, Deserialize)]
        struct Resistor(f32);

        let mut reg = PrefabRegistry::default();
        reg.register::<Net>()
            .register::<Resistor>()
            .register::<Use>();
        reg.register_net::<Net>();
        reg.register_global("gnd", serde_json::json!("gnd"));

        let lib: Library = serde_json::from_str(
            r#"{
            "branch": {
                "params": {"r": 1.0, "a":null, "b":null},
                "body": [
                    {"id":0,"components":{"Resistor":"$r","Net":["$a","mid"]}},
                    {"id":1,"components":{"Resistor":"$r","Net":["mid","$b"]}}
                ]
            },
            "pair": {
                "body": [
                    {"id":0,"components":{"Use":{"prefab":"branch","params":{"r":2.0,"a":"x","b":"@gnd"}}}},
                    {"id":1,"components":{"Use":{"prefab":"branch","params":{"r":3.0,"a":"y","b":"@gnd"}}}}
                ]
            }
        }"#,
        )
        .unwrap();
        reg.load_library(&lib).unwrap();
        let mut w = World::new();
        reg.spawn_class(&lib, "pair", &serde_json::Map::new(), "", &mut w);

        let nets: Vec<Vec<String>> = {
            let mut q = w.query::<&Net>();
            q.iter(&w).map(|n| n.0.clone()).collect()
        };
        assert_eq!(nets.len(), 4); // 2 branches × 2 resistors, all Use expanded
        let no_use = {
            let mut q = w.query::<&Use>();
            q.iter(&w).count()
        };
        assert_eq!(no_use, 0);

        let flat: std::collections::HashSet<&String> = nets.iter().flatten().collect();
        // the two branches' internal `mid` are qualified DISTINCT
        let mids: Vec<&&String> = flat.iter().filter(|s| s.ends_with("mid")).collect();
        assert_eq!(mids.len(), 2);
        assert!(flat.contains(&"0.mid".to_string()));
        assert!(flat.contains(&"1.mid".to_string()));
        // ports wired through (top level unqualified), global ground shared
        assert!(flat.contains(&"x".to_string()));
        assert!(flat.contains(&"y".to_string()));
        assert!(flat.contains(&"gnd".to_string()));
    }

    #[test]
    fn spawn_class_resolves_prefab_namespaces() {
        #[derive(Component, Clone, Debug, Serialize, Deserialize)]
        struct Resistor(f32);

        let mut reg = PrefabRegistry::default();
        reg.register::<Net>()
            .register::<Resistor>()
            .register::<Use>();
        reg.register_net::<Net>();

        let lib: Library = serde_json::from_str(
            r#"{
            "base::leaf": {
                "params": {"r": 1.0, "a":null, "b":null},
                "body": [
                    {"id":0,"components":{"Resistor":"$r","Net":["$a","$b"]}}
                ]
            },
            "user::pair": {
                "body": [
                    {"id":0,"components":{"Use":{"prefab":"base::leaf","params":{"r":2.0,"a":"x","b":"mid"}}}},
                    {"id":1,"components":{"Use":{"prefab":"base::leaf","params":{"r":3.0,"a":"mid","b":"y"}}}}
                ]
            },
            "user::top": {
                "body": [
                    {"id":0,"components":{"Use":{"prefab":"pair"}}}
                ]
            }
        }"#,
        )
        .unwrap();
        reg.load_library(&lib).unwrap();
        let mut world = World::new();
        reg.try_spawn_class(&lib, "user::top", &serde_json::Map::new(), "", &mut world)
            .unwrap();

        let rs = {
            let mut q = world.query::<&Resistor>();
            q.iter(&world).count()
        };
        assert_eq!(rs, 2);
    }

    #[test]
    fn registry_clone_and_patch_merge_are_overlay_like() {
        #[derive(Component, Clone, Debug, Serialize, Deserialize)]
        struct Resistor(f32);
        #[derive(Component, Clone, Debug, Serialize, Deserialize)]
        struct Capacitor(f32);

        let mut base = PrefabRegistry::default();
        base.register::<Resistor>();
        base.register_global("gnd", serde_json::json!("gnd"));
        base.register_global("rail", serde_json::json!("base_rail"));

        let mut patch = PrefabRegistry::default();
        patch.register::<Capacitor>();
        patch.register_net::<Net>();
        patch.register_global("rail", serde_json::json!("patched_rail"));

        let merged = base.merged_with(&patch);

        assert!(base.is_registered("Resistor"));
        assert!(!base.is_registered("Capacitor"));
        assert!(!base.net_components.contains("Net"));
        assert_eq!(base.globals["rail"], serde_json::json!("base_rail"));

        assert!(merged.is_registered("Resistor"));
        assert!(merged.is_registered("Capacitor"));
        assert!(merged.net_components.contains("Net"));
        assert_eq!(merged.globals["gnd"], serde_json::json!("gnd"));
        assert_eq!(merged.globals["rail"], serde_json::json!("patched_rail"));
    }

    #[test]
    fn global_overlay_resolves_chains_and_prefab_params() {
        #[derive(Component, Clone, Debug, Serialize, Deserialize)]
        struct Resistor(f64);

        let mut reg = PrefabRegistry::default();
        reg.register::<Resistor>();
        reg.register_global("base_r", serde_json::json!(2.5));
        let overlay = serde_json::json!({
            "scale": 4.0,
            "branch_r": "@base_r"
        })
        .as_object()
        .unwrap()
        .clone();
        reg.try_overlay_globals(&overlay).unwrap();

        let lib: Library = serde_json::from_value(serde_json::json!({
            "branch": {
                "params": {"r": null},
                "body": [{"id": 0, "components": {"Resistor": "$r"}}]
            }
        }))
        .unwrap();
        let params = serde_json::json!({"r": "@branch_r"})
            .as_object()
            .unwrap()
            .clone();
        reg.load_library(&lib).unwrap();
        let mut world = World::new();
        reg.try_spawn_class(&lib, "branch", &params, "x.", &mut world)
            .unwrap();

        let resistance = world.query::<&Resistor>().single(&world).unwrap();
        assert_eq!(resistance.0, 2.5);
        assert_eq!(reg.globals["scale"], serde_json::json!(4.0));
    }

    #[test]
    fn global_overlay_rejects_cycles() {
        let mut reg = PrefabRegistry::default();
        let overlay = serde_json::json!({"a": "@b", "b": "@a"})
            .as_object()
            .unwrap()
            .clone();
        let err = match reg.try_overlay_globals(&overlay) {
            Ok(_) => panic!("cyclic globals must be rejected"),
            Err(err) => err,
        };
        assert!(err.contains("cyclic global reference"));
    }

    #[test]
    fn try_register_duplicate_returns_error_not_panic() {
        #[derive(Component, Clone, Debug, Serialize, Deserialize)]
        struct Resistor(f32);

        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut reg = PrefabRegistry::default();
            reg.try_register::<Resistor>().unwrap();
            reg.try_register::<Resistor>().is_err()
        }));

        assert_eq!(outcome.unwrap(), true);
    }

    #[test]
    fn batch_registration_macros_are_fallible() {
        #[derive(Component, Clone, Debug, Serialize, Deserialize)]
        struct Resistor(f32);
        #[derive(Component, Clone, Debug, Serialize, Deserialize)]
        struct Capacitor(f32);

        fn build() -> Result<PrefabRegistry, String> {
            let mut reg = PrefabRegistry::default();
            crate::try_register_components!(reg, Resistor, Capacitor)?;
            crate::try_register_net_components!(reg, Net)?;
            Ok(reg)
        }

        let reg = build().unwrap();
        assert!(reg.is_registered("Resistor"));
        assert!(reg.is_registered("Capacitor"));
        assert!(reg.net_components.contains("Net"));
    }

    #[test]
    fn try_spawn_missing_param_returns_error_not_panic() {
        #[derive(Component, Clone, Debug, Serialize, Deserialize)]
        struct Resistor(f32);

        let mut reg = PrefabRegistry::default();
        reg.try_register::<Resistor>().unwrap();

        let lib: Library = serde_json::from_str(
            r#"{
            "bad": {
                "body": [
                    {"id":0,"components":{"Resistor":"$missing"}}
                ]
            }
        }"#,
        )
        .unwrap();

        let outcome =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| reg.load_library(&lib)));

        let err = outcome.unwrap().unwrap_err();
        assert!(err.contains("unknown param '$missing'"));
    }

    #[test]
    fn try_spawn_bad_component_json_returns_error_not_panic() {
        #[derive(Component, Clone, Debug, Serialize, Deserialize)]
        struct Resistor(f32);

        let mut reg = PrefabRegistry::default();
        reg.try_register::<Resistor>().unwrap();

        let lib: Library = serde_json::from_str(
            r#"{
            "bad": {
                "body": [
                    {"id":0,"components":{"Resistor":{"ohms": 5.0}}}
                ]
            }
        }"#,
        )
        .unwrap();

        let outcome =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| reg.load_library(&lib)));

        let err = outcome.unwrap().unwrap_err();
        assert!(err.contains("component 'Resistor' error"));
    }

    // ── feature: extract auto-detects boundary ports (extract.rs) ───────────
    #[test]
    fn extract_detects_boundary_ports() {
        let r = reg();
        let mut w = World::new();
        w.spawn((Local("src".into()), Net(vec!["in".into()])));
        let r1 = w
            .spawn((
                Resistor { ohms: 100.0 },
                Local("r1".into()),
                Net(vec!["in".into(), "mid".into()]),
            ))
            .id();
        let r2 = w
            .spawn((
                Resistor { ohms: 200.0 },
                Local("r2".into()),
                Net(vec!["mid".into(), "out".into()]),
            ))
            .id();
        w.spawn((Local("load".into()), Net(vec!["out".into()])));

        let def = r.extract(&w, &[r1, r2]);
        assert_eq!(def.params["in"], serde_json::Value::Null);
        assert_eq!(def.params["out"], serde_json::Value::Null);
        assert_eq!(def.body.len(), 2);
    }

    // ── feature: entity reference resolution (bind.rs) ──────────────────────
    #[test]
    fn entity_refs_resolve_global_and_local() {
        let mut w = World::new();
        // global: a tagged target + a probe referencing it by @ref
        let vsc = w.spawn(NameTag("vsc1".into())).id();
        let probe = w
            .spawn(Refs(HashMap::from([(
                "target".to_string(),
                "@ref:vsc1".to_string(),
            )])))
            .id();
        // local: siblings under one parent, referenced by $ref (Local name)
        let parent = w.spawn(()).id();
        let r1 = w.spawn((Local("r1".into()), ChildOf(parent))).id();
        let meter = w
            .spawn((
                Refs(HashMap::from([("t".to_string(), "$ref:r1".to_string())])),
                ChildOf(parent),
            ))
            .id();

        resolve_refs(&mut w);

        assert_eq!(w.get::<Bound>(probe).unwrap().0["target"], vsc);
        assert_eq!(w.get::<Bound>(meter).unwrap().0["t"], r1);
    }

    #[test]
    fn prefab_record_ids_support_self_and_internal_references() {
        let mut reg = PrefabRegistry::default();
        reg.register::<Refs>();
        let lib: Library = serde_json::from_str(
            r#"{
                "pair": {
                    "body": [
                        {"id":8,"components":{"Refs":{"self":"$id:8"}}},
                        {"id":9,"components":{"Refs":{"target":"$id:8"}}}
                    ]
                }
            }"#,
        )
        .unwrap();
        reg.load_library(&lib).unwrap();
        let mut world = World::new();
        let err = reg.try_spawn_class(&lib, "bad", &serde_json::Map::new(), "", &mut world);
        assert!(err.is_err());
        let first = reg.spawn_class(&lib, "pair", &serde_json::Map::new(), "0.", &mut world);
        let second = reg.spawn_class(&lib, "pair", &serde_json::Map::new(), "1.", &mut world);
        resolve_refs(&mut world);

        assert_eq!(world.get::<Bound>(first[0]).unwrap().0["self"], first[0]);
        assert_eq!(world.get::<Bound>(first[1]).unwrap().0["target"], first[0]);
        assert_eq!(world.get::<Bound>(second[0]).unwrap().0["self"], second[0]);
        assert_eq!(
            world.get::<Bound>(second[1]).unwrap().0["target"],
            second[0]
        );
    }

    // ── feature: patch verb on a named entity (patch.rs) ────────────────────
    #[test]
    fn patch_sets_and_attaches_on_named_entity() {
        #[derive(Component, Clone, PartialEq, Debug, Serialize, Deserialize)]
        struct Probe {
            ch: String,
        }

        let mut reg = PrefabRegistry::default();
        reg.register::<Resistor>().register::<Probe>();

        let mut w = World::new();
        let e = w
            .spawn((NameTag("load1".into()), Resistor { ohms: 50.0 }))
            .id();

        let patch: Patch = serde_json::from_str(
            r#"{
            "target": "@ref:load1",
            "set": { "Resistor": { "ohms": 75.0 } },
            "attach": { "Probe": { "ch": "i" } }
        }"#,
        )
        .unwrap();

        assert!(reg.apply_patch(&mut w, &patch));
        assert_eq!(w.get::<Resistor>(e).unwrap().ohms, 75.0);
        assert_eq!(w.get::<Probe>(e).unwrap().ch, "i");
    }

    // ── feature: three-phase RLC user-requested test ────────────────────────
    #[test]
    fn test_rlc_three_phase_grounded() {
        #[derive(Component, Clone, PartialEq, Debug, Serialize, Deserialize)]
        struct Resistor(f32);
        #[derive(Component, Clone, PartialEq, Debug, Serialize, Deserialize)]
        struct Inductor(f32);
        #[derive(Component, Clone, PartialEq, Debug, Serialize, Deserialize)]
        struct Capacitor(f32);

        let mut reg = PrefabRegistry::default();
        reg.register::<Resistor>()
            .register::<Inductor>()
            .register::<Capacitor>()
            .register::<Net>()
            .register::<Use>();
        reg.register_net::<Net>();
        reg.register_global("gnd", serde_json::json!("gnd"));

        let lib: Library = serde_json::from_str(
            r#"{
            "rlc_parallel": {
                "params": {"r": 10.0, "l": 0.1, "c": 0.01, "in": "n_in", "out": "n_out"},
                "body": [
                    {"id":0,"components":{"Resistor":"$r","Net":["$in","$out"]}},
                    {"id":1,"components":{"Inductor":"$l","Net":["$in","$out"]}},
                    {"id":2,"components":{"Capacitor":"$c","Net":["$in","$out"]}}
                ]
            },
            "rlc_3phase": {
                "params": {
                    "r_a": 1.0, "l_a": 0.01, "c_a": 0.001,
                    "r_b": 1.0, "l_b": 0.01, "c_b": 0.001,
                    "r_c": 1.0, "l_c": 0.01, "c_c": 0.001
                },
                "body": [
                    {"id":0,"components":{"Use":{"prefab":"rlc_parallel","params":{"r":"$r_a","l":"$l_a","c":"$c_a","in":"phase_a","out":"@gnd"}}}},
                    {"id":1,"components":{"Use":{"prefab":"rlc_parallel","params":{"r":"$r_b","l":"$l_b","c":"$c_b","in":"phase_b","out":"@gnd"}}}},
                    {"id":2,"components":{"Use":{"prefab":"rlc_parallel","params":{"r":"$r_c","l":"$l_c","c":"$c_c","in":"phase_c","out":"@gnd"}}}}
                ]
            }
        }"#,
        )
        .unwrap();

        reg.load_library(&lib).unwrap();
        let mut world = World::new();

        let overrides: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(r#"{"r_a": 5.0, "l_a": 0.05, "c_a": 0.005}"#).unwrap();

        reg.spawn_class(&lib, "rlc_3phase", &overrides, "sub.", &mut world);

        // Verify spawned components in TargetWorld
        let mut r_values: Vec<f32> = world
            .query::<&Resistor>()
            .iter(&world)
            .map(|r| r.0)
            .collect();
        let l_values: Vec<f32> = world
            .query::<&Inductor>()
            .iter(&world)
            .map(|l| l.0)
            .collect();
        let c_values: Vec<f32> = world
            .query::<&Capacitor>()
            .iter(&world)
            .map(|c| c.0)
            .collect();

        assert_eq!(r_values.len(), 3);
        assert_eq!(l_values.len(), 3);
        assert_eq!(c_values.len(), 3);

        // We override phase A to (5.0, 0.05, 0.005), phase B and C should remain defaults (1.0, 0.01, 0.001)
        r_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(r_values, vec![1.0, 1.0, 5.0]);
    }

    #[test]
    fn load_library_rejects_unknown_components() {
        let mut reg = PrefabRegistry::default();
        let lib: Library = serde_json::from_value(serde_json::json!({
            "bad": {
                "body": [
                    {"id": 0, "components": {"NotRegistered": 1.0}}
                ]
            }
        }))
        .unwrap();

        let err = reg.load_library(&lib).unwrap_err();
        assert!(err.contains("unknown component 'NotRegistered'"));
    }

    #[test]
    fn builtin_local_survives_without_explicit_registration() {
        let mut reg = PrefabRegistry::default();
        reg.register::<Refs>();
        let lib: Library = serde_json::from_value(serde_json::json!({
            "pair": {
                "body": [
                    {"id": 0, "components": {}},
                    {"id": 1, "parent": 0, "components": {"Local": "target"}},
                    {"id": 2, "parent": 0, "components": {"Refs": {"target": "$ref:target"}}}
                ]
            }
        }))
        .unwrap();

        reg.load_library(&lib).unwrap();
        let mut world = World::new();
        let spawned = reg.spawn_class(&lib, "pair", &serde_json::Map::new(), "", &mut world);
        resolve_refs(&mut world);

        assert_eq!(spawned.len(), 1);
        let mut refs = world.query::<(Entity, &Refs)>();
        let (holder, _) = refs.single(&world).unwrap();
        let bound = world.get::<Bound>(holder).unwrap();
        assert!(bound.0.contains_key("target"));
        assert!(world.get::<Local>(bound.0["target"]).is_some());
    }

    #[test]
    fn prefab_world_template_roundtrip_preserves_semantics() {
        let mut reg = reg();
        let lib: Library = serde_json::from_value(serde_json::json!({
            "leaf": {
                "params": {"a": null},
                "body": [
                    {
                        "id": 0,
                        "components": {
                            "Local": "leaf",
                            "Net": ["$a", "mid"]
                        }
                    }
                ]
            },
            "branch": {
                "params": {"r": 1.0, "a": null, "b": "out"},
                "body": [
                    {
                        "id": 10,
                        "components": {
                            "Local": "root",
                            "Resistor": {"ohms": "$r"},
                            "Net": ["$a", "@gnd"]
                        }
                    },
                    {
                        "id": 11,
                        "parent": 10,
                        "components": {
                            "Local": "child",
                            "Use": {
                                "prefab": "leaf",
                                "params": {
                                    "a": "$a",
                                    "b": "mid",
                                    "r": "$r"
                                }
                            }
                        }
                    }
                ]
            }
        }))
        .unwrap();

        reg.load_library(&lib).unwrap();

        let exported = reg.try_export_class("branch").unwrap();
        assert_eq!(exported.params, lib["branch"].params);
        assert_eq!(exported.body.len(), 2);

        let root = exported.body.iter().find(|record| record.id == 10).unwrap();
        assert_eq!(root.parent, None);
        assert_eq!(root.components["Local"], serde_json::json!("root"));
        assert_eq!(
            root.components["Resistor"],
            serde_json::json!({"ohms": "$r"})
        );
        assert_eq!(root.components["Net"], serde_json::json!(["$a", "@gnd"]));

        assert!(
            exported
                .body
                .iter()
                .all(|record| !record.components.contains_key("ChildOf"))
        );

        let child = exported.body.iter().find(|record| record.id == 11).unwrap();
        assert_eq!(child.parent, Some(10));
        assert_eq!(child.components["Local"], serde_json::json!("child"));
        assert_eq!(
            child.components["Use"],
            serde_json::json!({
                "prefab": "leaf",
                "params": {"a": "$a", "b": "mid", "r": "$r"}
            })
        );

        let leaf = reg.try_export_class("leaf").unwrap();
        assert_eq!(
            leaf.body[0].components["Net"],
            serde_json::json!(["$a", "mid"])
        );
    }

    #[test]
    fn structured_param_paths_deep_merge_and_spawn() {
        let mut reg = reg();
        reg.register_net::<Net>();
        reg.register_global("gnd", serde_json::json!("gnd"));

        let lib: Library = serde_json::from_value(serde_json::json!({
            "phase_branch": {
                "params": {
                    "physical": {"r": null, "l": 0.2},
                    "ports": {"p": null, "n": "@gnd"}
                },
                "body": [
                    {
                        "id": 0,
                        "components": {
                            "Resistor": {"ohms": "$physical.r"},
                            "Net": ["$ports.p", "$ports.n"]
                        }
                    }
                ]
            },
            "load": {
                "params": {
                    "physical": {"r": 10.0, "l": 0.2},
                    "ports": {"a": null, "n": "@gnd"}
                },
                "body": [
                    {
                        "id": 0,
                        "components": {
                            "Use": {
                                "prefab": "phase_branch",
                                "params": {
                                    "physical": {
                                        "r": "$physical.r"
                                    },
                                    "ports": {
                                        "p": "$ports.a",
                                        "n": "$ports.n"
                                    }
                                }
                            }
                        }
                    }
                ]
            }
        }))
        .unwrap();

        reg.load_library(&lib).unwrap();

        let exported = reg.try_export_class("load").unwrap();
        assert_eq!(
            exported.body[0].components["Use"]["params"]["physical"]["r"],
            serde_json::json!("$physical.r")
        );
        assert_eq!(
            exported.body[0].components["Use"]["params"]["ports"]["p"],
            serde_json::json!("$ports.a")
        );

        let params = serde_json::json!({
            "physical": {"r": 25.0},
            "ports": {"a": "bus_a"}
        })
        .as_object()
        .unwrap()
        .clone();
        let mut world = World::new();
        reg.try_spawn_class(&lib, "load", &params, "x.", &mut world)
            .unwrap();

        let resistor = world.query::<&Resistor>().single(&world).unwrap();
        assert_eq!(resistor.ohms, 25.0);
        let net = world.query::<&Net>().single(&world).unwrap();
        assert_eq!(net.0, vec!["bus_a".to_string(), "gnd".to_string()]);
    }

    #[test]
    fn structured_param_paths_report_missing_nested_required_values() {
        let mut reg = reg();
        reg.register_net::<Net>();

        let lib: Library = serde_json::from_value(serde_json::json!({
            "load": {
                "params": {
                    "ports": {"a": null, "n": "gnd"}
                },
                "body": [
                    {"id": 0, "components": {"Net": ["$ports.a", "$ports.n"]}}
                ]
            }
        }))
        .unwrap();

        reg.load_library(&lib).unwrap();
        let mut world = World::new();
        let err = reg
            .try_spawn_class(&lib, "load", &serde_json::Map::new(), "", &mut world)
            .unwrap_err();
        assert!(err.contains("required param 'ports.a'"));
    }
}
