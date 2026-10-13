use tt_core::vcam::scene::Scene;

const LIMIT: usize = 200;

/// Undo by observation: whatever changed the scene, the change becomes one step once the user lets go
/// (pointer up, no text field focused), so a drag or a typed name is a single undo.
#[derive(Default)]
pub struct History {
    stable: Option<Scene>,
    undo: Vec<Scene>,
    redo: Vec<Scene>,
}

impl History {
    pub fn observe(&mut self, scene: &Scene, interacting: bool) {
        match &self.stable {
            None => self.stable = Some(scene.clone()),
            Some(stable) if stable != scene && !interacting => {
                let before = self.stable.replace(scene.clone()).expect("checked");
                self.undo.push(before);
                if self.undo.len() > LIMIT {
                    self.undo.remove(0);
                }
                self.redo.clear();
            }
            Some(_) => {}
        }
    }

    pub fn undo(&mut self, scene: &mut Scene) -> bool {
        self.observe(scene, false);
        let Some(previous) = self.undo.pop() else { return false };
        self.redo.push(std::mem::replace(scene, previous));
        self.stable = Some(scene.clone());
        true
    }

    pub fn redo(&mut self, scene: &mut Scene) -> bool {
        let Some(next) = self.redo.pop() else { return false };
        self.undo.push(std::mem::replace(scene, next));
        self.stable = Some(scene.clone());
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Forgets everything, e.g. after switching scenes.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drag_is_one_step_and_redo_restores_it() {
        let mut history = History::default();
        let mut scene = Scene::default();
        history.observe(&scene, false);
        for x in 0..10 {
            scene.width = 1000 + x;
            history.observe(&scene, true);
        }
        history.observe(&scene, false);
        assert!(history.undo(&mut scene));
        assert_eq!(scene.width, Scene::default().width);
        assert!(!history.can_undo());
        assert!(history.redo(&mut scene));
        assert_eq!(scene.width, 1009);
    }

    #[test]
    fn an_uncommitted_change_is_undone_first() {
        let mut history = History::default();
        let mut scene = Scene::default();
        history.observe(&scene, false);
        scene.fps = 15;
        history.observe(&scene, true);
        assert!(history.undo(&mut scene));
        assert_eq!(scene.fps, 30);
    }
}
