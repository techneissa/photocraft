use super::*;
use crate::smartselect_cmds::{MODEL_FAILED, MODEL_NO_MEMORY};
use photocraft_geom::Rect;

fn region() -> Region {
    Region { bbox: Rect::new(0, 0, 1, 1), mask: vec![255] }
}

#[test]
fn only_the_ai_choices_try_the_full_model_and_only_when_it_is_installed() {
    for engine in [SubjectEngine::Auto, SubjectEngine::Model] {
        assert!(tried_first(engine, true), "{engine:?}");
        assert!(!tried_first(engine, false), "{engine:?} without the file goes straight on to BiRefNet-lite");
    }
    for engine in [SubjectEngine::MacOs, SubjectEngine::Sam3, SubjectEngine::Classic] {
        assert!(!tried_first(engine, true), "{engine:?} keeps its own engine");
    }
}

#[test]
fn unit_tests_never_run_the_full_model_unless_one_is_installed_for_the_test() {
    assert!(!installed());
}

#[test]
fn a_matte_ends_the_chain() {
    for engine in [SubjectEngine::Auto, SubjectEngine::Model] {
        assert_eq!(step(engine, Ok(Some(region()))).unwrap(), Step::Done(Some(region())));
    }
}

#[test]
fn no_subject_is_the_answer_of_the_model_choice_but_not_of_auto() {
    assert_eq!(step(SubjectEngine::Model, Ok(None)).unwrap(), Step::Done(None));
    assert_eq!(step(SubjectEngine::Auto, Ok(None)).unwrap(), Step::Next { classic: None, other: None });
}

#[test]
fn a_full_model_that_cannot_run_falls_through_with_a_note_and_never_errors() {
    for e in [
        MlError::Backend("load".into()),
        MlError::Unsupported,
        MlError::Download("x".into()),
        MlError::ModelMissing { model: photocraft_ml::Model::Birefnet, path: "p".into() },
    ] {
        for engine in [SubjectEngine::Auto, SubjectEngine::Model] {
            assert_eq!(step(engine, Err(e.clone())).unwrap(), Step::Next { classic: Some(MODEL_FAILED), other: Some(FULL_FAILED) }, "{e:?}");
        }
    }
    let low = MlError::LowMemory { have: 8 << 30, needs: 16 << 30 };
    assert_eq!(step(SubjectEngine::Auto, Err(low)).unwrap(), Step::Next { classic: Some(MODEL_NO_MEMORY), other: Some(FULL_NO_MEMORY) });
}

#[test]
fn cancel_and_a_budget_overrun_end_the_command() {
    assert!(matches!(step(SubjectEngine::Auto, Err(MlError::Cancelled)), Err(EngineError::Cancelled)));
    assert!(matches!(step(SubjectEngine::Model, Err(MlError::BadInput("too big".into()))), Err(EngineError::Other(m)) if m == "too big"));
}
