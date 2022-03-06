use std::iter::Peekable;
use std::path::{Component, Path, PathBuf};

use red4ext_rs::interop::IsoRED;
use red4ext_rs::prelude::*;
use stroke::{CubicBezier, Point, PointN};

define_plugin! {
    name: "svg4reds",
    author: "jekky",
    version: 0:1:0,
    on_register: {
        register_function!("Svg.Core.LoadSvg", load_svg);
    }
}

fn load_svg(name: String) -> Ref<ffi::IScriptable> {
    try_load_svg(&name).unwrap_or_else(Ref::null)
}

fn try_load_svg(name: &str) -> Option<Ref<ffi::IScriptable>> {
    let contents = std::fs::read_to_string(get_svg_path(name)?).ok()?;
    let tree = usvg::Tree::from_str(&contents, &usvg::Options::default().to_ref()).ok()?;
    convert_node(tree.root())
}

fn convert_node(node: usvg::Node) -> Option<Ref<ffi::IScriptable>> {
    match &*node.borrow() {
        usvg::NodeKind::Svg(svg) => {
            let view = svg.view_box;
            let canv = call!("inkCanvasWidget::New;" () -> Ref<ffi::IScriptable>);
            let size = Vector2::new(view.rect.width() as f32, view.rect.height() as f32);
            call!(canv, "SetSize" (size) -> ());

            for child in node.children() {
                if let Some(c) = convert_node(child) {
                    call!(canv, "AddChildWidget" (c) -> ());
                }
            }
            Some(canv)
        }
        usvg::NodeKind::Defs => None,
        // TODO: implement gradients
        usvg::NodeKind::LinearGradient(_) => None,
        usvg::NodeKind::RadialGradient(_) => None,
        // TODO: implement clip path
        usvg::NodeKind::ClipPath(_) => None,
        usvg::NodeKind::Mask(_) => None,
        usvg::NodeKind::Pattern(_) => None,
        usvg::NodeKind::Path(path) => Some(convert_path(path)),
        usvg::NodeKind::Image(_) => None,
        usvg::NodeKind::Group(group) => {
            let canv = call!("inkCanvasWidget::New;" () -> Ref<ffi::IScriptable>);
            let (tx, ty) = group.transform.get_translate();
            let (sx, sy) = group.transform.get_scale();

            call!(canv, "SetTranslation" (tx as f32, ty as f32) -> ());
            call!(canv, "SetScale" (Vector2::new(sx as f32, sy as f32)) -> ());
            call!(canv, "SetOpacity" (group.opacity.value() as f32) -> ());

            for child in node.children() {
                if let Some(node) = convert_node(child) {
                    call!(canv, "AddChildWidget" (node) -> ());
                }
            }
            Some(canv)
        }
    }
}

fn convert_path(path: &usvg::Path) -> Ref<ffi::IScriptable> {
    let mut it = path.data.iter().peekable();
    let canv = call!("inkCanvasWidget::New;" () -> Ref<ffi::IScriptable>);
    let (tx, ty) = path.transform.get_translate();
    let (sx, sy) = path.transform.get_scale();
    let visible = path.visibility == usvg::Visibility::Visible;

    call!(canv, "SetTranslation" (tx as f32, ty as f32) -> ());
    call!(canv, "SetScale" (Vector2::new(sx as f32, sy as f32)) -> ());
    call!(canv, "SetVisible" (visible) -> ());

    while it.len() > 0 {
        let set = extract_vertices(&mut it);
        let shape = create_shape(path, set.is_closed);
        for vert in set.vertices {
            call!(shape, "AddVertex" (vert) -> ());
        }
        call!(canv, "AddChildWidget" (shape) -> ());
    }
    canv
}

fn create_shape(path: &usvg::Path, closed: bool) -> Ref<ffi::IScriptable> {
    let shape = call!("inkShapeWidget::New;" () -> Ref<ffi::IScriptable>);
    call!(shape, "SetUseNineSlice" (true) -> ());

    match &path.fill {
        Some(fill) => match fill.paint {
            usvg::Paint::Color(color) => {
                call!(shape, "SetTintColor" (Color::rgba_u8(color.red, color.green, color.blue, 255)) -> ());
                call!(shape, "SetFillOpacity" (fill.opacity.value() as f32) -> ());
            }
            usvg::Paint::Link(_) => {}
        },
        None => {}
    }

    match &path.stroke {
        Some(stroke) => {
            call!(shape, "SetLineThickness" (stroke.width.value() as f32) -> ());
            call!(shape, "SetBorderOpacity" (stroke.opacity.value() as f32) -> ());

            let cap = match stroke.linecap {
                _ if closed => EndCapStyle::Joined,
                usvg::LineCap::Round => EndCapStyle::Round,
                usvg::LineCap::Square => EndCapStyle::Square,
                usvg::LineCap::Butt => EndCapStyle::Butt,
            };
            let joint = match stroke.linejoin {
                usvg::LineJoin::Round => JointStyle::Round,
                usvg::LineJoin::Bevel => JointStyle::Bevel,
                usvg::LineJoin::Miter => JointStyle::Miter,
            };

            call!(shape, "SetEndCapStyle" (cap) -> ());
            call!(shape, "SetJointStyle" (joint) -> ());

            match stroke.paint {
                usvg::Paint::Color(color) => {
                    call!(shape, "SetBorderColor" (Color::rgba_u8(color.red, color.green, color.blue, 255)) -> ());
                }
                usvg::Paint::Link(_) => {}
            }
        }
        None => {}
    }
    let shape_variant = match (&path.fill, &path.stroke) {
        (Some(_), None) => ShapeVariant::Fill,
        (None, Some(_)) => ShapeVariant::Border,
        (Some(_), Some(_)) => ShapeVariant::FillAndBorder,
        (None, None) => ShapeVariant::Fill,
    };
    call!(shape, "SetShapeVariant" (shape_variant) -> ());

    shape
}

fn extract_vertices<'a, I: Iterator<Item = &'a usvg::PathSegment>>(
    paths: &mut Peekable<I>,
) -> VerticeSet {
    let mut vertices = match paths.next_if(|seg| matches!(seg, usvg::PathSegment::MoveTo { .. })) {
        Some(usvg::PathSegment::MoveTo { x, y }) => {
            vec![Vector2::new(*x as f32, *y as f32)]
        }
        _ => vec![],
    };
    while let Some(seg) = paths.next_if(|seg| !matches!(seg, usvg::PathSegment::MoveTo { .. })) {
        match seg {
            usvg::PathSegment::LineTo { x, y } => {
                vertices.push(Vector2::new((*x) as f32, (*y) as f32));
            }
            usvg::PathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                if let Some(start) = vertices.last().cloned() {
                    let curve = render_bezier(
                        PointN::new([start.x as f64, start.y as f64]),
                        PointN::new([*x1, *y1]),
                        PointN::new([*x2, *y2]),
                        PointN::new([*x, *y]),
                    );
                    vertices.extend(curve.into_iter().skip(1));
                }
            }
            usvg::PathSegment::ClosePath => {
                if !vertices.is_empty() && vertices.first() == vertices.last() {
                    vertices.pop();
                }
                return VerticeSet::closed(vertices);
            }
            usvg::PathSegment::MoveTo { .. } => return VerticeSet::open(vertices),
        }
    }
    VerticeSet::open(vertices)
}

fn render_bezier(
    start: PointN<f64, 2>,
    ctrl1: PointN<f64, 2>,
    ctrl2: PointN<f64, 2>,
    end: PointN<f64, 2>,
) -> Vec<Vector2> {
    const BEZIER_STEPS: usize = 12;
    let mut vertices = Vec::with_capacity(BEZIER_STEPS);

    let bezier = CubicBezier::new(start, ctrl1, ctrl2, end);
    for step in 0..BEZIER_STEPS {
        let step = step as f64 * (1f64 / BEZIER_STEPS as f64);
        let point = bezier.eval_casteljau(step);
        let vec = Vector2::new(point.axis(0) as f32, point.axis(1) as f32);
        vertices.push(vec);
    }
    vertices
}

fn get_svg_path(path: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let path: &Path = path.as_ref();
    if path
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
    {
        let path = exe
            .parent()?
            .parent()?
            .parent()?
            .join("r6")
            .join("svg")
            .join(path)
            .with_extension("svg");
        Some(path)
    } else {
        None
    }
}

#[derive(Debug)]
struct VerticeSet {
    vertices: Vec<Vector2>,
    is_closed: bool,
}

impl VerticeSet {
    fn open(vertices: Vec<Vector2>) -> Self {
        Self {
            vertices,
            is_closed: false,
        }
    }

    fn closed(vertices: Vec<Vector2>) -> Self {
        Self {
            vertices,
            is_closed: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
#[repr(C)]
struct Vector2 {
    x: f32,
    y: f32,
}

impl Vector2 {
    #[inline]
    fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl IsoRED for Vector2 {
    #[inline]
    fn type_name() -> &'static str {
        "Vector2"
    }
}

#[derive(Debug, Clone, Default)]
#[repr(C)]
struct Color {
    red: f32,
    green: f32,
    blue: f32,
    alpha: f32,
}

impl Color {
    #[inline]
    fn rgba_u8(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red: red as f32 / 255f32,
            green: green as f32 / 255f32,
            blue: blue as f32 / 255f32,
            alpha: alpha as f32 / 255f32,
        }
    }
}

impl IsoRED for Color {
    #[inline]
    fn type_name() -> &'static str {
        "HDRColor"
    }
}

#[derive(Debug)]
#[repr(i64)]
enum ShapeVariant {
    Fill = 0,
    Border = 1,
    FillAndBorder = 2,
}

impl Default for ShapeVariant {
    #[inline]
    fn default() -> Self {
        Self::Fill
    }
}

impl IsoRED for ShapeVariant {
    #[inline]
    fn type_name() -> &'static str {
        "inkEShapeVariant"
    }
}

#[derive(Debug)]
#[repr(i64)]
enum JointStyle {
    Miter = 0,
    Bevel = 1,
    Round = 2,
}

impl Default for JointStyle {
    #[inline]
    fn default() -> Self {
        Self::Miter
    }
}

impl IsoRED for JointStyle {
    #[inline]
    fn type_name() -> &'static str {
        "inkEJointStyle"
    }
}

#[derive(Debug)]
#[repr(i64)]
enum EndCapStyle {
    Butt = 0,
    Square = 1,
    Round = 2,
    Joined = 3,
}

impl Default for EndCapStyle {
    #[inline]
    fn default() -> Self {
        Self::Butt
    }
}

impl IsoRED for EndCapStyle {
    #[inline]
    fn type_name() -> &'static str {
        "inkEEndCapStyle"
    }
}
