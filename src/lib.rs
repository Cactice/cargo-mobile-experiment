use env_logger;
use log::info;
use mobile_entry_point::mobile_entry_point;
use pollster::FutureExt;
use std::{borrow::Cow, time::Instant};
use wgpu::{
    Adapter, Device, PipelineLayout, Queue, RenderBundle, RenderPipeline, ShaderModule, Surface,
    SurfaceConfiguration, TextureFormat,
};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};
type OptionalInit = (
    Option<wgpu::Instance>,
    Option<Adapter>,
    Option<ShaderModule>,
    Option<PipelineLayout>,
    Option<SurfaceConfiguration>,
    Option<Surface>,
    Option<Device>,
    Option<Queue>,
    Option<RenderPipeline>,
);

const TARGET_FPS: u64 = 60;
fn init_none() -> OptionalInit {
    (None, None, None, None, None, None, None, None, None)
}

async fn init(
    window: &Window,
) -> (
    wgpu::Instance,
    Adapter,
    ShaderModule,
    PipelineLayout,
    SurfaceConfiguration,
    Surface,
    Device,
    Queue,
    RenderPipeline,
) {
    let size = window.inner_size();
    // conditional backends as workaround for https://github.com/gfx-rs/wgpu/issues/2384
    #[cfg(all(target_arch = "x86_64", target_os = "android"))]
    let backends = wgpu::Backends::GL;
    #[cfg(not(all(target_arch = "x86_64", target_os = "android")))]
    let backends = wgpu::Backends::all();
    let instance = wgpu::Instance::new(backends);
    let surface = unsafe { instance.create_surface(&window) };
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            // Request an adapter which can render to our surface
            compatible_surface: Some(&surface),
        })
        .await
        .expect("Failed to find an appropriate adapter");

    // Create the logical device and command queue
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: wgpu::Features::empty(),
                limits: adapter.limits(),
            },
            None,
        )
        .await
        .expect("Failed to create device");

    // Load the shaders from disk
    let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });

    let texture_format = surface
        .get_preferred_format(&adapter)
        .expect("Failed to get preferred_format");

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[texture_format.into()],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: texture_format,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::Fifo,
    };

    surface.configure(&device, &config);
    (
        instance,
        adapter,
        shader,
        pipeline_layout,
        config,
        surface,
        device,
        queue,
        render_pipeline,
    )
}
async fn init_some(window: &Window) -> OptionalInit {
    let (
        instance,
        adapter,
        shader,
        pipeline_layout,
        config,
        surface,
        device,
        queue,
        render_pipeline,
    ) = init(window).await;
    (
        Some(instance),
        Some(adapter),
        Some(shader),
        Some(pipeline_layout),
        Some(config),
        Some(surface),
        Some(device),
        Some(queue),
        Some(render_pipeline),
    )
}

async fn run(event_loop: EventLoop<()>, window: Window) {
    #[cfg(target_os = "android")]
    let (
        mut instance,
        mut adapter,
        mut shader,
        mut pipeline_layout,
        mut config,
        mut surface,
        mut device,
        mut queue,
        mut render_pipeline,
    ) = init_none();
    #[cfg(not(target_os = "android"))]
    let (
        mut instance,
        mut adapter,
        mut shader,
        mut pipeline_layout,
        mut config,
        mut surface,
        mut device,
        mut queue,
        mut render_pipeline,
    ) = init_some(&window).await;
    let start_time = Instant::now();
    event_loop.run(move |event, _, control_flow| {
        // Have the closure take ownership of the resources.
        // `event_loop.run` never returns, therefore we must do this to ensure
        // the resources are properly cleaned up.
        let _ = (&instance, &adapter, &shader, &pipeline_layout);

        *control_flow = ControlFlow::Wait;
        match event {
            Event::Resumed => {
                (
                    instance,
                    adapter,
                    shader,
                    pipeline_layout,
                    config,
                    surface,
                    device,
                    queue,
                    render_pipeline,
                ) = init_some(&window).block_on();
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                if let (Some(config), Some(surface), Some(device)) =
                    (config.as_mut(), surface.as_ref(), device.as_ref())
                {
                    // Reconfigure the surface with the new size
                    config.width = size.width;
                    config.height = size.height;
                    surface.configure(&device, &config);
                }
            }
            Event::RedrawRequested(_) => {
                if let (Some(surface), Some(device), Some(render_pipeline), Some(queue)) =
                    (&surface, &device, &render_pipeline, &queue)
                {
                    let frame = surface
                        .get_current_texture()
                        .expect("Failed to acquire next swap chain texture");
                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    let mut encoder = device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                    {
                        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: None,
                            color_attachments: &[wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                                    store: true,
                                },
                            }],
                            depth_stencil_attachment: None,
                        });
                        rpass.set_pipeline(&render_pipeline);
                        rpass.draw(0..3, 0..1);
                    }

                    queue.submit(Some(encoder.finish()));
                    frame.present();
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => {}
        }
        match *control_flow {
            ControlFlow::Exit => (),
            _ => {
                /*
                 * Grab window handle from the display (untested - based on API)
                 */
                window.request_redraw();
                /*
                 * Below logic to attempt hitting TARGET_FPS.
                 * Basically, sleep for the rest of our milliseconds
                 */
                let elapsed_time_millis =
                    Instant::now().duration_since(start_time).as_millis() as u64;

                let wait_millis = match 1000 / TARGET_FPS >= elapsed_time_millis {
                    true => 1000 / TARGET_FPS - elapsed_time_millis,
                    false => 0,
                };
                let new_inst = start_time + std::time::Duration::from_millis(wait_millis);
                *control_flow = ControlFlow::WaitUntil(new_inst);
            }
        }
    });
}

#[cfg(target_os = "android")]
fn init_logging() {
    android_logger::init_once(
        android_logger::Config::default()
            .with_min_level(log::Level::Info)
            .with_tag("cargo-mobile"),
    );
}

#[cfg(not(target_os = "android"))]
fn init_logging() {
    env_logger::init();
}

#[mobile_entry_point]
fn main() {
    init_logging();
    let event_loop = EventLoop::new();
    let window = winit::window::Window::new(&event_loop).unwrap();
    #[cfg(not(target_arch = "wasm32"))]
    {
        pollster::block_on(run(event_loop, window));
    }
    #[cfg(target_arch = "wasm32")]
    {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        use winit::platform::web::WindowExtWebSys;
        // On wasm, append the canvas to the document body
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| doc.body())
            .and_then(|body| {
                body.append_child(&web_sys::Element::from(window.canvas()))
                    .ok()
            })
            .expect("couldn't append canvas to document body");
        wasm_bindgen_futures::spawn_local(run(event_loop, window));
    }
}
