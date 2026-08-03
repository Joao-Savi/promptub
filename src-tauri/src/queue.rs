use crate::discover::title_fingerprint;
use crate::player;
use crate::queue_refill;
use crate::state::SharedState;
use crate::youtube::Video;
use serde::Serialize;
use std::collections::HashSet;
use tauri::State;

pub struct Queue {
    items: Vec<Video>,
    current: isize,
}

impl Queue {
    pub fn new() -> Self {
        Self {
            items: vec![],
            current: -1,
        }
    }

    pub fn add(&mut self, v: Video) {
        self.items.push(v);
        if self.current < 0 {
            self.current = 0;
        }
    }

    pub fn play_now(&mut self, v: Video) {
        self.items = vec![v];
        self.current = 0;
    }

    pub fn load_playlist(&mut self, items: Vec<Video>) {
        if items.is_empty() {
            return;
        }
        self.items = items;
        self.current = 0;
    }

    pub fn remove_at(&mut self, index: usize) -> Result<(), String> {
        if index >= self.items.len() {
            return Err("Indice invalido".into());
        }
        self.items.remove(index);
        if self.items.is_empty() {
            self.current = -1;
            return Ok(());
        }
        let cur = self.current as usize;
        if index < cur {
            self.current -= 1;
        } else if index == cur && cur >= self.items.len() {
            self.current = self.items.len() as isize - 1;
        }
        Ok(())
    }

    pub fn jump_to(&mut self, index: usize) -> Option<Video> {
        if index >= self.items.len() {
            return None;
        }
        self.current = index as isize;
        Some(self.items[index].clone())
    }

    pub fn current_video(&self) -> Option<Video> {
        if self.current >= 0 && (self.current as usize) < self.items.len() {
            Some(self.items[self.current as usize].clone())
        } else {
            None
        }
    }

    /// Proximas `count` faixas apos a que esta tocando (nao inclui a atual).
    pub fn upcoming_from_current(&self, count: usize) -> Vec<Video> {
        if self.current < 0 || self.items.is_empty() || count == 0 {
            return vec![];
        }
        let start = self.current as usize + 1;
        self.items.iter().skip(start).take(count).cloned().collect()
    }

    pub fn remaining_after_current(&self) -> usize {
        if self.current < 0 || self.items.is_empty() {
            return 0;
        }
        self.items
            .len()
            .saturating_sub(self.current as usize + 1)
    }

    pub fn existing_ids(&self) -> HashSet<String> {
        self.items.iter().map(|v| v.id.clone()).collect()
    }

    pub fn purge_where(&mut self, pred: &dyn Fn(&Video) -> bool) -> usize {
        let before = self.items.len();
        let cur_idx = self.current;
        self.items.retain(|v| !pred(v));
        let removed = before.saturating_sub(self.items.len());
        if self.items.is_empty() {
            self.current = -1;
        } else if cur_idx >= 0 {
            let cur = cur_idx as usize;
            if cur >= self.items.len() {
                self.current = self.items.len() as isize - 1;
            }
        }
        removed
    }

    pub fn append_unique(&mut self, items: Vec<Video>) -> usize {
        let mut seen_ids = self.existing_ids();
        let mut seen_fps: HashSet<String> = self
            .items
            .iter()
            .map(title_fingerprint)
            .collect();
        let mut added = 0usize;
        for video in items {
            let fp = title_fingerprint(&video);
            if seen_ids.contains(&video.id) || seen_fps.contains(&fp) {
                continue;
            }
            seen_ids.insert(video.id.clone());
            seen_fps.insert(fp);
            self.items.push(video);
            added += 1;
        }
        added
    }

    pub fn next(&mut self) -> Option<Video> {
        if self.items.is_empty() {
            return None;
        }
        if self.current < self.items.len() as isize - 1 {
            self.current += 1;
            return Some(self.items[self.current as usize].clone());
        }
        None
    }

    pub fn prev(&mut self) -> Option<Video> {
        if self.items.is_empty() {
            return None;
        }
        if self.current > 0 {
            self.current -= 1;
            return Some(self.items[self.current as usize].clone());
        }
        None
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.current = -1;
    }

    pub fn snapshot(&self) -> QueueSnapshot {
        QueueSnapshot {
            items: self.items.clone(),
            current: self.current,
        }
    }
}

#[derive(Clone, Serialize)]
pub struct QueueSnapshot {
    pub items: Vec<Video>,
    pub current: isize,
}

#[tauri::command]
pub fn enqueue(state: State<'_, SharedState>, video: Video) -> Result<(), String> {
    state.queue.lock().add(video);
    crate::stream::prewarm_queue_ahead(&state);
    queue_refill::maybe_refill_queue(&state);
    Ok(())
}

#[tauri::command]
pub fn get_queue(state: State<'_, SharedState>) -> Result<QueueSnapshot, String> {
    Ok(state.queue.lock().snapshot())
}

#[tauri::command]
pub fn clear_queue(state: State<'_, SharedState>) -> Result<(), String> {
    state.queue.lock().clear();
    Ok(())
}

#[tauri::command]
pub fn remove_queue_item(state: State<'_, SharedState>, index: usize) -> Result<(), String> {
    state.queue.lock().remove_at(index)?;
    crate::stream::prewarm_queue_ahead(&state);
    Ok(())
}

#[tauri::command]
pub fn load_queue(state: State<'_, SharedState>, items: Vec<Video>) -> Result<(), String> {
    state.queue.lock().load_playlist(items);
    crate::stream::prewarm_queue_ahead(&state);
    queue_refill::maybe_refill_queue(&state);
    Ok(())
}

#[tauri::command]
pub fn play_queue_item(
    state: State<'_, SharedState>,
    index: usize,
) -> Result<Option<Video>, String> {
    let video = state.queue.lock().jump_to(index);
    if let Some(ref v) = video {
        state.set_last_video(v.clone());
        player::track_play(state.inner(), v);
    }
    Ok(video)
}
