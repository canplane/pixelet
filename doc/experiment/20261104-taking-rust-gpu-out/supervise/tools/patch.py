# Adds a mutant loop to the scratch copy's tests/world.rs, ahead of the test's own run.
import sys
p=sys.argv[1]; s=open(p).read()
marker='    let source = [words, hand, ENTRY].join("\\n");\n'
assert marker in s
block=r'''
    if let Ok(list) = std::env::var("MUTANTS") {
        let raw = std::fs::read_to_string(std::env::var("WORLD_WGSL").unwrap()).unwrap();
        let text = std::fs::read_to_string(list).unwrap();
        for line in text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
            let parts: Vec<&str> = line.split(" ||| ").collect();
            let name = parts[0];
            let (mut w, mut h) = (words.to_string(), raw.clone());
            if parts.len() == 3 {
                if h.contains(parts[1]) {
                    h = h.replacen(parts[1], parts[2], 1);
                } else if w.contains(parts[1]) {
                    w = w.replacen(parts[1], parts[2], 1);
                } else {
                    println!("MUT {name}: NO MATCH");
                    continue;
                }
            }
            let source = [w.as_str(), h.as_str(), ENTRY].join("\n");
            let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
            let refused = pollster::block_on(scope.pop());
            if let Some(e) = refused {
                println!("MUT {name}: REFUSED {}", e.to_string().lines().next().unwrap_or(""));
                continue;
            }
            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: None,
                module: &module,
                entry_point: Some("world_asked_cs"),
                compilation_options: Default::default(),
                cache: None,
            });
            let mut by_op = [0u32; 4];
            for (a, t) in asks.asked.chunks(16384 * ASKED_WORDS).zip(asks.told.chunks(16384 * TOLD_WORDS)) {
                let asked_buf = buffer(a, store);
                let told_buf = buffer(&vec![0u32; t.len()], store);
                let entries = [(1, &clip_buf), (2, &ground_buf), (4, &palette_buf), (5, &shard_buf), (6, &placed_buf), (7, &pool_buf), (12, &asked_buf), (13, &told_buf)]
                    .map(|(b, buf): (u32, &wgpu::Buffer)| wgpu::BindGroupEntry { binding: b, resource: buf.as_entire_binding() });
                let group = device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &pipeline.get_bind_group_layout(0), entries: &entries });
                let mut e = device.create_command_encoder(&Default::default());
                {
                    let mut pass = e.begin_compute_pass(&Default::default());
                    pass.set_pipeline(&pipeline);
                    pass.set_bind_group(0, &group, &[]);
                    pass.dispatch_workgroups(((a.len() / ASKED_WORDS) as u32).div_ceil(64), 1, 1);
                }
                queue.submit([e.finish()]);
                let got = read(&device, &queue, &told_buf);
                for (k, (want, have)) in t.chunks(TOLD_WORDS).zip(got.chunks(TOLD_WORDS)).enumerate() {
                    if want != have {
                        by_op[a[k * ASKED_WORDS] as usize] += 1;
                    }
                }
            }
            let n: u32 = by_op.iter().sum();
            println!("MUT {} {name}: differ {n} {by_op:?}", if n == 0 { "SURVIVED" } else { "killed" });
        }
        return;
    }
'''
s=s.replace(marker,block+marker)
open(p,'w').write(s)
