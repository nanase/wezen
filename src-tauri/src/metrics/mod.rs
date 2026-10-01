//! Windows から値を取る。
//!
//! | 項目 | 取り出し元 |
//! |---|---|
//! | CPU | `GetSystemTimes` |
//! | Memory | `GlobalMemoryStatusEx` のコミット済みの量（物理メモリ + ページファイル） |
//! | I/O | `NtQuerySystemInformation` の I/O の累計（R+O は Read と Other の和） |
//! | GPU | `\GPU Engine(*)`、`\GPU Adapter Memory(*)`、DXGI |
//! | Disk | 物理ドライブごとの `IOCTL_DISK_PERFORMANCE` |
//! | Network | `GetIfTable2` |

mod cpu;
mod disk;
pub mod gpu;
mod io;
mod memory;
mod net;
mod pdh;

use crate::config::Settings;
use crate::sampler::{Sample, Source};
use pdh::{Counter, Query};

struct GpuCounters {
    query: Query,
    engine: Option<Counter>,
    memory: Option<Counter>,
}

impl GpuCounters {
    fn open() -> Option<Self> {
        let query = Query::open()?;
        Some(Self {
            engine: query.add(r"\GPU Engine(*)\Utilization Percentage"),
            memory: query.add(r"\GPU Adapter Memory(*)\Dedicated Usage"),
            query,
        })
    }
}

pub struct Collector {
    cpu: cpu::Cpu,
    io: io::Io,
    disk: disk::Disk,
    net: net::Net,
    gpu: Option<GpuCounters>,
    adapters: Vec<gpu::Adapter>,
}

impl Collector {
    pub fn new() -> Self {
        Self {
            cpu: cpu::Cpu::default(),
            io: io::Io::default(),
            disk: disk::Disk::default(),
            net: net::Net::default(),
            gpu: GpuCounters::open(),
            adapters: gpu::adapters(),
        }
    }
}

impl Source for Collector {
    fn sample(&mut self, settings: &Settings) -> Sample {
        let mut s = Sample {
            cpu: self.cpu.sample(),
            ..Sample::default()
        };
        if let Some((used, total)) = memory::sample() {
            s.mem_used = Some(used);
            s.mem_total = Some(total);
        }
        if let Some((ro, w)) = self.io.sample() {
            s.io_ro = Some(ro);
            s.io_w = Some(w);
        }
        if let Some((r, w)) = self.disk.sample() {
            s.disk_r = Some(r);
            s.disk_w = Some(w);
        }
        if let Some((rx, tx)) = self.net.sample() {
            s.net_r = Some(rx);
            s.net_s = Some(tx);
        }

        let Some(adapter) = gpu::pick(&self.adapters, settings.gpu.as_deref()) else {
            return s;
        };
        s.vram_total = Some(adapter.memory);
        let Some(c) = &self.gpu else {
            return s;
        };
        if !c.query.collect() {
            return s;
        }
        // 毎秒の量なので、初回は値がそろわず空になる
        let engines = c.engine.map(|c| c.values()).unwrap_or_default();
        if !engines.is_empty() {
            s.gpu = Some(gpu::usage(&engines, &adapter.id));
        }
        if let Some(counter) = c.memory {
            s.vram_used = Some(gpu::dedicated(&counter.values(), &adapter.id));
        }
        s
    }
}
