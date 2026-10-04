// STAT: versioned, bounded snapshots of kernel state for observers (MC-10.2). Records never carry authority, task
// memory contents or physical addresses of task memory.
use super::*;

const HEADER: usize = core::mem::size_of::<StatHeader>();

// Writes records into the caller's buffer after the header; counts the ones that did not fit.
struct Out<'a> { space: &'a paging::Space, base: usize, capacity: usize, size: usize, count: usize, total: usize, fault: bool }
impl Out<'_> {
    fn push<T: Copy>(&mut self, record: T) {
        self.total += 1;
        let offset = HEADER + self.count * self.size;
        if offset + self.size > self.capacity || self.fault { return; }
        let bytes = unsafe { core::slice::from_raw_parts((&record as *const T).cast::<u8>(), self.size) };
        if copy_out(self.space, self.base + offset, bytes) { self.count += 1; } else { self.fault = true; }
    }
    fn finish(self) -> Result<usize, usize> {
        let header = StatHeader { version: STAT_VERSION, record_size: self.size as u32, count: self.count as u32, total: self.total as u32 };
        let bytes = unsafe { core::slice::from_raw_parts((&header as *const StatHeader).cast::<u8>(), HEADER) };
        if self.fault || !copy_out(self.space, self.base, bytes) { Err(ERR_INVALID) } else { Ok(self.count) }
    }
}

// Largest block the kernel arena could still allocate (page-aligned), found by trying: the allocator keeps no list of
// its free blocks to read. Runs under the scheduler lock, so nothing allocates meanwhile.
fn largest_free(free: usize) -> usize {
    let (mut low, mut high) = (0usize, free.next_multiple_of(4096) + 4096);
    while high - low > 4096 {
        let mid = (low + (high - low) / 2) & !4095;
        if mid <= low { break; }
        let layout = core::alloc::Layout::from_size_align(mid, 4096).unwrap();
        let mut heap = crate::ALLOCATOR.lock();
        match heap.allocate_first_fit(layout) { Ok(block) => { unsafe { heap.deallocate(block, layout); } low = mid; } Err(()) => high = mid }
    }
    low
}

fn wait_of(state: State, running: bool, tasks: &[Option<Task>; SLOTS]) -> (u8, u32) {
    if running { return (WAIT_RUNNING, 0); }
    match state {
        State::BlockedSend(ep) => (WAIT_SEND, ep as u32),
        State::BlockedRecv(ep) => (WAIT_RECEIVE, ep as u32),
        State::BlockedReply(server) => (WAIT_REPLY, tasks[server].as_ref().map_or(0, |t| t.pid as u32)),
        State::Sleeping(_) => (WAIT_SLEEP, 0),
        State::BlockedIrq(irq) => (WAIT_IRQ, irq as u32),
        State::BlockedFlush => (WAIT_FLUSH, 0),
        State::Exited => (WAIT_EXITED, 0),
        State::Ready | State::Empty => (WAIT_NONE, 0),
    }
}

impl Scheduler {
    pub(super) fn stat(&self, slot: usize, request: &SyscallMailbox) -> Result<usize, usize> {
        let (class, argument) = (request.arg1, request.msg[1]);
        let size = match class {
            STAT_TASKS => core::mem::size_of::<StatTask>(), STAT_CPUS => core::mem::size_of::<StatCpu>(), STAT_MEMORY => core::mem::size_of::<StatMemory>(),
            STAT_PHYSMAP => core::mem::size_of::<StatPhys>(), STAT_VMAP => core::mem::size_of::<StatRegion>(), STAT_CAPS => core::mem::size_of::<StatCap>(),
            STAT_ENDPOINTS => core::mem::size_of::<StatEndpoint>(), STAT_IRQS => core::mem::size_of::<StatIrq>(), STAT_DEVICES => core::mem::size_of::<StatDevice>(),
            _ => return Err(ERR_INVALID),
        };
        if request.msg[0] < HEADER { return Err(ERR_INVALID); }
        let mut out = Out { space: &self.tasks[slot].as_ref().unwrap().space, base: request.arg2, capacity: request.msg[0], size, count: 0, total: 0, fault: false };
        let pid_of = |index: usize| self.tasks[index].as_ref().map_or(0, |t| t.pid);
        // The task that uses a capability: the holder of its most recently derived copy (init keeps the copies it granted,
        // to restart a driver; the driver's copy derives from it), and how many live tasks hold a copy.
        let holder = |test: &dyn Fn(&Capability) -> bool| {
            let (mut newest, mut pid, mut holders) = (0u64, 0u64, 0u32);
            for task in self.tasks.iter().flatten().filter(|t| t.state != State::Exited) {
                let mut held = false;
                for (index, cap) in task.cspace.iter().enumerate() {
                    if !cap.as_ref().is_some_and(test) { continue; }
                    held = true;
                    if task.nodes[index].id >= newest { newest = task.nodes[index].id; pid = task.pid; }
                }
                holders += held as u32;
            }
            (pid, holders)
        };
        match class {
            STAT_TASKS => for (index, task) in self.tasks.iter().enumerate().skip(1) {
                let Some(task) = task else { continue };
                let (wait, wait_on) = wait_of(task.state, self.current.contains(&index), &self.tasks);
                let mut name = [0u8; NAME_MAX]; name[..task.name.len as usize].copy_from_slice(&task.name.bytes[..task.name.len as usize]);
                let alive = task.state != State::Exited;
                out.push(StatTask {
                    pid: task.pid, parent: task.parent.map_or(0, |p| p.1), name, wait, cpu: task.cpu as u8, service: task.service as u8, screen: task.screen.is_some() as u8, wait_on,
                    run_ns: task.run_ns, runs: task.runs, ticks: task.ticks, calls: task.calls, sends: task.sends, receives: task.receives, started_ns: task.started_ns,
                    heap_bytes: task.heap.bytes() as u64, heap_blocks: task.heap.block_count() as u32, caps: task.cspace.iter().flatten().count() as u32,
                    shared_bytes: task.heap.shared_bytes() as u64, retained_bytes: task.heap.retained as u64,
                    image_bytes: task._image.len() as u64, stack_bytes: task._stack.len() as u64, screen_bytes: task.screen.as_ref().map_or(0, |s| s.len() as u64),
                    quota_tasks: task.quota_tasks as u16, used_tasks: if alive { self.used_tasks(index) as u16 } else { 0 },
                    quota_endpoints: task.quota_endpoints as u16, used_endpoints: if alive { self.used_endpoints(index) as u16 } else { 0 },
                    band: task.band, throttled: (task.budget_ns != 0 && task.consumed >= task.budget_ns) as u8, focus: (index == self.foreground) as u8, reserved: 0,
                    budget_ns: task.budget_ns, period_ns: task.period_ns,
                    kernel_bytes: (task.context.len() + task._exit.len() + task.abi.len() + task.space.table_count() * 4096) as u64,
                });
            },
            STAT_CPUS => for index in 0..cpu::COUNT.load(Ordering::Acquire) {
                let a = &self.accounting;
                out.push(StatCpu { apic_id: cpu::apic_id(index), online: cpu::ONLINE[index].load(Ordering::Acquire) as u32, ticks: cpu::TICKS[index].load(Ordering::Relaxed),
                    busy_ns: a.busy_ns[index], idle_ns: a.idle_ns[index], interrupts: a.interrupts[index], switches: a.switches[index], current_pid: pid_of(self.current[index]) });
            },
            STAT_MEMORY => {
                let (used, free) = { let heap = crate::ALLOCATOR.lock(); (heap.used(), heap.free()) };
                let mut m = StatMemory { arena: self.boot.heap_len as u64, used: used as u64, free: free as u64, dma_limit: DMA_LIMIT as u64, objects_limit: DETACHED_MAX_BYTES as u64,
                    largest_free: if argument == 1 { largest_free(free) as u64 } else { 0 }, tasks_limit: MAX_TASKS as u32, endpoints_limit: (ENDPOINTS - FIRST_ENDPOINT) as u32, ..Default::default() };
                for task in self.tasks.iter().flatten() {
                    m.images += task._image.len() as u64; m.stacks += task._stack.len() as u64;
                    m.task_pages += (task.context.len() + task._exit.len() + task.abi.len()) as u64;
                    m.screens += task.screen.as_ref().map_or(0, |s| s.len() as u64); m.heaps += task.heap.bytes() as u64; m.tasks += 1;
                    m.page_tables += (task.space.table_count() * 4096) as u64; m.shared += task.heap.shared_bytes() as u64;
                }
                m.objects = self.orphans.iter().map(|o| o.region.len() as u64).sum();
                m.dma = self.dma.iter().map(|r| r.len() as u64).sum();
                m.endpoints = (FIRST_ENDPOINT..ENDPOINTS).filter(|&e| self.endpoints[e]).count() as u64;
                out.push(m);
            }
            STAT_PHYSMAP => {
                for index in 0..self.boot.memory_map_len { out.push(unsafe { *self.boot.memory_map.add(index) }); }
                let pages = |bytes: usize| bytes.div_ceil(4096) as u64;
                extern "C" { static __kernel_end: u8; }
                let (kernel_start, kernel_end) = (crate::_start as *const () as usize, core::ptr::addr_of!(__kernel_end) as usize);
                out.push(StatPhys { kind: PHYS_KERNEL, index: 0, start: kernel_start as u64, pages: pages(kernel_end - kernel_start) });
                out.push(StatPhys { kind: PHYS_ARENA, index: 0, start: self.boot.heap_ptr as u64, pages: pages(self.boot.heap_len) });
                out.push(StatPhys { kind: PHYS_FRAMEBUFFER, index: 0, start: self.boot.fb_ptr as u64, pages: pages(frame_bytes(&self.boot)) });
                out.push(StatPhys { kind: PHYS_AP_TRAMPOLINE, index: 0, start: self.boot.ap_trampoline as u64, pages: 1 });
                for (index, image) in self.boot.programs.iter().enumerate() { out.push(StatPhys { kind: PHYS_BOOT_IMAGE, index: index as u32, start: image.data as u64, pages: pages(image.len) }); }
                for (index, device) in self.devices.iter().enumerate() {
                    for bar in device.bars.iter().filter(|b| b.size != 0 && !b.io) { out.push(StatPhys { kind: PHYS_PCI_BAR, index: index as u32, start: bar.base, pages: pages(bar.size as usize) }); }
                }
            }
            STAT_VMAP | STAT_CAPS => {
                let target = self.find(argument as u64).ok_or(ERR_NOT_FOUND)?;
                let task = self.tasks[target].as_ref().unwrap();
                if class == STAT_VMAP {
                    task.space.regions(|start, size, writable, executable, device| {
                        let kind = match start {
                            a if a >= paging::USER_HEAP => match task.heap.kind_at(a) { Some((true, _)) => REGION_HEAP, Some((false, true)) => REGION_DEVICE, _ if device => REGION_DEVICE, _ => REGION_SHARED },
                            a if a >= paging::USER_EXIT => REGION_EXIT,
                            a if a >= paging::USER_MAILBOX => REGION_MAILBOX,
                            a if a >= paging::USER_INFO => REGION_INFO,
                            a if a >= paging::USER_SCREEN => REGION_SCREEN,
                            a if a >= paging::USER_STACK => REGION_STACK,
                            _ => REGION_IMAGE,
                        };
                        // The page below the stack is left unmapped on purpose: an overflow faults there.
                        if kind == REGION_STACK && start == paging::USER_STACK { out.push(StatRegion { start: (start - 4096) as u64, size: 4096, kind: REGION_GUARD, flags: 0 }); }
                        out.push(StatRegion { start: start as u64, size: size as u64, kind, flags: REGION_READ | if writable { REGION_WRITE } else { 0 } | if executable { REGION_EXECUTE } else { 0 } });
                    });
                } else {
                    for index in 1..CAP_SLOTS {
                        let Some(cap) = task.cspace[index] else { continue };
                        let endpoint = if let Capability::Endpoint(ep, ..) = cap { ep as u32 } else { 0 };
                        let (kind, rights, size, badge) = match cap {
                            Capability::Endpoint(_, rights, badge) => (CAP_KIND_ENDPOINT, rights as u32, 0, badge as u32),
                            Capability::Memory(_, size, rights) => (CAP_KIND_MEMORY, rights as u32, size as u64, 0),
                            Capability::Dma(_, size) => (CAP_KIND_DMA, 0, size as u64, 0),
                            Capability::Mmio(_, size) => (CAP_KIND_MMIO, 0, size as u64, 0),
                            Capability::IoPorts(base, count) => (CAP_KIND_PORTS, base as u32, count as u64, 0),
                            Capability::Interrupt(irq) => (CAP_KIND_IRQ, irq as u32, 0, 0),
                            Capability::Input => (CAP_KIND_INPUT, 0, 0, 0), Capability::Display => (CAP_KIND_DISPLAY, 0, 0, 0),
                            Capability::Spawn => (CAP_KIND_SPAWN, 0, 0, 0), Capability::Reply(..) => (CAP_KIND_REPLY, 0, 0, 0),
                            Capability::Platform => (CAP_KIND_PLATFORM, 0, 0, 0), Capability::Control => (CAP_KIND_CONTROL, 0, 0, 0),
                            Capability::Restart => (CAP_KIND_RESTART, 0, 0, 0), Capability::Observe => (CAP_KIND_OBSERVE, 0, 0, 0),
                        };
                        let generation = if index < SLOT_DYNAMIC { 0 } else { task.generations[index] };
                        out.push(StatCap { slot: index as u32, generation, kind: kind as u32, rights, size, badge, endpoint, node: task.nodes[index].id, parent: task.nodes[index].parent });
                    }
                }
            }
            STAT_ENDPOINTS => for ep in (FIRST_ENDPOINT..ENDPOINTS).filter(|&e| self.endpoints[e]) {
                let live = || self.tasks.iter().flatten().filter(|t| t.state != State::Exited);
                let c = self.accounting.endpoint[ep];
                out.push(StatEndpoint {
                    index: ep as u32,
                    receivers: live().filter(|t| t.cspace.iter().flatten().any(|c| matches!(c, Capability::Endpoint(id, rights, _) if *id == ep && rights & CAP_READ != 0))).count() as u32,
                    waiting_senders: live().filter(|t| t.state == State::BlockedSend(ep)).count() as u32,
                    waiting_receivers: live().filter(|t| t.state == State::BlockedRecv(ep)).count() as u32,
                    creator: self.endpoint_owner[ep].map_or(0, |o| o.1), messages: c.messages, busy: c.busy, timeouts: c.timeouts,
                    server: holder(&|c| matches!(c, Capability::Endpoint(id, rights, _) if *id == ep && rights & CAP_READ != 0)).0,
                    holders: holder(&|c| matches!(c, Capability::Endpoint(id, ..) if *id == ep)).1,
                    irq: self.irq_bind.iter().position(|bound| *bound == Some(ep)).map_or(0, |line| line as u32),
                });
            },
            // PIC lines 1..15, then the MSI-X vectors handed out (lines 16..31).
            STAT_IRQS => for line in (1..16u8).filter(|&l| l != 2).chain((0..MSI_VECTORS).filter(|&i| self.msi[i].is_some()).map(|i| (MSI_FIRST + i) as u8)) {
                let (holder, holders) = holder(&|c| *c == Capability::Interrupt(line));
                out.push(StatIrq { line: line as u32, endpoint: self.irq_bind[line as usize].map_or(0, |e| e as u32), masked: interrupts::irq_masked(line) as u32, holders,
                    holder, count: self.accounting.irqs[line as usize] });
            },
            STAT_DEVICES => for device in self.devices.iter() {
                let (mut bar_sizes, mut io_bars) = ([0u64; 6], 0u32);
                for (index, (size, bar)) in bar_sizes.iter_mut().zip(device.bars.iter()).enumerate() { *size = bar.size; if bar.io && bar.size != 0 { io_bars |= 1 << index; } }
                let owns = |c: &Capability| device.bars.iter().any(|b| b.size != 0 && match *c {
                    Capability::Mmio(base, _) => !b.io && base as u64 >= b.base && (base as u64) < b.base + b.size,
                    Capability::IoPorts(base, _) => b.io && base as u64 >= b.base && (base as u64) < b.base + b.size,
                    _ => false,
                });
                out.push(StatDevice { class: device.class, irq: device.irq as u32, bar_sizes, holder: holder(&owns).0, location: device.location(), io_bars });
            },
            _ => unreachable!(),
        }
        out.finish()
    }
}
