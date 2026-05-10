from __future__ import annotations

from dataclasses import dataclass

from manim import (
    DOWN,
    LEFT,
    RIGHT,
    UP,
    Arrow,
    Create,
    FadeIn,
    FadeOut,
    FadeTransform,
    GrowArrow,
    LaggedStart,
    Line,
    Rectangle,
    RoundedRectangle,
    Scene,
    SurroundingRectangle,
    Text,
    VGroup,
    WHITE,
)
from manim import config as manim_config


manim_config.background_color = "#07111F"

FONT_FAMILY = "Verdana"

FILE_CHUNKS = 6

FRAME_STROKE = "#7F8EA3"
MUTED_TEXT = "#AAB8C8"
VIEWER_COLOR = "#F6BD60"
READER_COLOR = "#84A59D"
WRITER_COLOR = "#F28482"
UPSTREAM_COLOR = "#4CC9F0"
CACHE_COLOR = "#90BE6D"
SUBREAD_COLOR = "#FFD166"
SUBREAD_SLOT_COLOR = "#B9852A"
MISS_COLOR = "#F94144"
HIT_COLOR = "#6E9B4B"

Text.set_default(font=FONT_FAMILY)


@dataclass(frozen=True)
class PanelTheme:
    stroke: str
    fill: str


class FileColumn(VGroup):
    def __init__(
        self,
        title: str,
        *,
        chunk_count: int,
        body_color: str,
        label_color: str = WHITE,
        fill_chunk_indices: set[int] | None = None,
        chunk_stroke_color: str | None = None,
    ) -> None:
        super().__init__()
        fill_chunk_indices = fill_chunk_indices or set()
        chunk_stroke_color = chunk_stroke_color or FRAME_STROKE

        title_text = Text(title, font_size=18, color=label_color, weight="BOLD")
        body = Rectangle(height=4.4, width=1.3, stroke_color=FRAME_STROKE, stroke_width=2)
        title_text.next_to(body, UP, buff=0.14)

        chunk_height = body.height / chunk_count
        chunks = VGroup()
        filled = {}
        for idx in range(chunk_count):
            top = body.get_top()[1] - (idx * chunk_height)
            rect = Rectangle(
                height=chunk_height,
                width=body.width,
                stroke_color=chunk_stroke_color,
                stroke_width=1.5,
                fill_color=body_color,
                fill_opacity=0.15 if idx not in fill_chunk_indices else 0.78,
            )
            rect.move_to([body.get_center()[0], top - (chunk_height / 2), 0])
            chunks.add(rect)
            if idx in fill_chunk_indices:
                filled[idx] = rect

        self.body = body
        self.title_text = title_text
        self.chunks = chunks
        self.filled = filled

        self.add(body, chunks, title_text)

    def chunk_rect(self, idx: int) -> Rectangle:
        return self.chunks[idx]

    def chunk_center(self, idx: int):
        return self.chunk_rect(idx).get_center()

    def move_body_to(self, point) -> "FileColumn":
        self.shift(point - self.body.get_center())
        return self


class RoleCard(VGroup):
    def __init__(
        self,
        title: str,
        lines: list[str],
        theme: PanelTheme,
        *,
        width: float = 2.65,
        height: float = 1.0,
        title_size: int = 22,
        line_size: int = 16,
        rounded: bool = False,
        center_lines: bool = False,
    ) -> None:
        super().__init__()
        body_cls = RoundedRectangle if rounded else Rectangle
        body = body_cls(
            width=width,
            height=height,
            stroke_color=theme.stroke,
            fill_color=theme.fill,
            fill_opacity=0.18,
            stroke_width=2,
            **({"corner_radius": 0.14} if rounded else {}),
        )
        heading = Text(title, font_size=title_size, color=theme.stroke, weight="BOLD")
        heading.move_to(body.get_top() + DOWN * 0.21)

        line_group = VGroup(*[Text(line, font_size=line_size, color=WHITE) for line in lines])
        if center_lines:
            line_group.arrange(DOWN, buff=0.06)
        else:
            line_group.arrange(DOWN, aligned_edge=LEFT, buff=0.06)
        line_group.move_to(body.get_center() + DOWN * 0.16)

        self.body = body
        self.heading = heading
        self.lines = line_group
        self.add(body, heading, line_group)


class GeneralReadScene(Scene):
    def construct(self) -> None:
        viewer = RoleCard(
            "Viewer",
            ["requests bytes"],
            PanelTheme(VIEWER_COLOR, "#3D2B1F"),
            width=2.7,
            height=1.2,
            rounded=True,
            center_lines=True,
        )
        viewer.move_to([4.2, 0.15, 0])
        viewer.shift(UP * 0.95)

        axis = Line(start=[-3.65, -2.0, 0], end=[-3.65, 2.1, 0], color=FRAME_STROKE, stroke_width=2)
        axis_label = Text("offset / chunks", font_size=18, color=MUTED_TEXT)
        axis_label.rotate(1.5708)
        axis_label.next_to(axis, LEFT, buff=0.45)

        y_ticks = VGroup()
        y_labels = VGroup()
        for idx in range(FILE_CHUNKS + 1):
            ratio = idx / FILE_CHUNKS
            y = axis.get_top()[1] - (axis.height * ratio)
            tick = Line(start=[-3.77, y, 0], end=[-3.53, y, 0], color=FRAME_STROKE, stroke_width=2)
            y_ticks.add(tick)
            label = Text(str(idx), font_size=16, color=MUTED_TEXT)
            label.next_to(tick, LEFT, buff=0.16)
            y_labels.add(label)

        upstream = FileColumn(
            "Upstream Object",
            chunk_count=FILE_CHUNKS,
            body_color=UPSTREAM_COLOR,
            fill_chunk_indices=set(range(FILE_CHUNKS)),
            chunk_stroke_color="#B9D7E3",
        )
        cache = FileColumn(
            "Sparse Cache",
            chunk_count=FILE_CHUNKS,
            body_color=CACHE_COLOR,
            fill_chunk_indices={0},
        )
        upstream.move_body_to([-2.45, 0.05, 0])
        cache.move_body_to([0.35, 0.05, 0])

        reader_tag = Text("Reader", font_size=22, color=READER_COLOR, weight="BOLD")
        reader_tag.next_to(upstream.title_text, UP, buff=0.08)
        writer_tag = Text("Writer", font_size=22, color="#7FAA5A", weight="BOLD")
        writer_tag.next_to(cache.title_text, UP, buff=0.08)
        reader_size = Text("size: 100%", font_size=16, color=UPSTREAM_COLOR, weight="BOLD")
        reader_size.next_to(upstream.body, DOWN, buff=0.24)
        writer_size_20 = Text("size: 20%", font_size=16, color=CACHE_COLOR, weight="BOLD")
        writer_size_20.next_to(cache.body, DOWN, buff=0.24)
        writer_size_40 = Text("size: 40%", font_size=16, color=CACHE_COLOR, weight="BOLD")
        writer_size_40.move_to(writer_size_20)
        writer_size_60 = Text("size: 60%", font_size=16, color=CACHE_COLOR, weight="BOLD")
        writer_size_60.move_to(writer_size_20)

        miss_chunk = 2
        prefetch_chunk = miss_chunk + 1
        buffered_badge = self.badge("buffered read", VIEWER_COLOR)
        buffered_badge.next_to(viewer, DOWN, buff=0.22)

        miss_badge = self.badge("cache miss", MISS_COLOR)
        miss_badge.move_to([
            (upstream.body.get_right()[0] + cache.body.get_left()[0]) / 2,
            cache.chunk_rect(miss_chunk).get_center()[1] + 0.48,
            0,
        ])
        hit_badge = self.badge("cache hit", HIT_COLOR)
        hit_badge.move_to([
            (upstream.body.get_right()[0] + cache.body.get_left()[0]) / 2,
            cache.chunk_rect(prefetch_chunk).get_center()[1] + 0.48,
            0,
        ])
        prefetch_badge = self.badge("prefetch", "#7FAA5A")
        prefetch_badge.move_to([
            (upstream.body.get_right()[0] + cache.body.get_left()[0]) / 2,
            cache.chunk_rect(prefetch_chunk).get_center()[1] + 0.95,
            0,
        ])

        miss_story_target = miss_badge.copy()
        prefetch_story_target = prefetch_badge.copy()
        hit_story_target = hit_badge.copy()
        story_arrow_1 = Text("→", font_size=30, color=MUTED_TEXT, weight="BOLD")
        story_arrow_2 = Text("→", font_size=30, color=MUTED_TEXT, weight="BOLD")
        story_flow = VGroup(miss_story_target, story_arrow_1, prefetch_story_target, story_arrow_2, hit_story_target)
        story_flow.arrange(RIGHT, buff=0.16)
        story_flow.move_to([0.35, -3.25, 0])

        cache_request_slice = self.make_slice(cache.chunk_rect(miss_chunk), 0.42, 0.18, SUBREAD_COLOR)
        second_slice = self.make_slice(cache.chunk_rect(prefetch_chunk), 0.42, 0.18, HIT_COLOR)

        ask_cache_arrow = Arrow(
            buffered_badge.get_left() + UP * 0.1,
            cache.body.get_right() + UP * 0.24,
            buff=0.18,
            color=VIEWER_COLOR,
            stroke_width=4,
        )
        miss_to_upstream = Arrow(
            cache.body.get_left() + UP * 0.34,
            upstream.body.get_right() + UP * 0.34,
            buff=0.2,
            color=MISS_COLOR,
            stroke_width=4,
        )
        fill_cache_arrow = Arrow(
            upstream.body.get_right() + DOWN * 0.05,
            cache.body.get_left() + DOWN * 0.05,
            buff=0.2,
            color=READER_COLOR,
            stroke_width=4,
        )
        miss_return_to_viewer = Arrow(
            cache.body.get_right() + DOWN * 0.24,
            buffered_badge.get_left() + DOWN * 0.1,
            buff=0.18,
            color=UPSTREAM_COLOR,
            stroke_width=4,
        )
        hit_return_to_viewer = Arrow(
            cache.body.get_right() + DOWN * 0.24,
            buffered_badge.get_left() + DOWN * 0.1,
            buff=0.18,
            color=CACHE_COLOR,
            stroke_width=4,
        )

        full_chunk_transfer = upstream.chunk_rect(miss_chunk).copy().set_fill(UPSTREAM_COLOR, opacity=0.88)
        full_chunk_transfer.set_stroke(UPSTREAM_COLOR, width=2)
        full_chunk_transfer.scale(0.94)
        full_chunk_transfer.move_to(upstream.chunk_center(miss_chunk))
        prefetch_chunk_transfer = upstream.chunk_rect(prefetch_chunk).copy().set_fill(UPSTREAM_COLOR, opacity=0.88)
        prefetch_chunk_transfer.set_stroke(UPSTREAM_COLOR, width=2)
        prefetch_chunk_transfer.scale(0.94)
        prefetch_chunk_transfer.move_to(upstream.chunk_center(prefetch_chunk))

        cached_chunk_outline = SurroundingRectangle(cache.chunk_rect(miss_chunk), color=CACHE_COLOR, buff=-0.02, stroke_width=3)
        prefetched_chunk_outline = SurroundingRectangle(cache.chunk_rect(prefetch_chunk), color=CACHE_COLOR, buff=-0.02, stroke_width=3)

        viewer_receive_target = viewer.get_center() + DOWN * 0.12
        viewer_request_slot = self.make_slice(cache.chunk_rect(miss_chunk), 0.5, 0.18, SUBREAD_SLOT_COLOR)
        viewer_request_slot.set_fill(SUBREAD_SLOT_COLOR, opacity=0.62)
        viewer_request_slot.set_stroke(SUBREAD_COLOR, width=2)
        viewer_request_slot.move_to(viewer_receive_target)
        viewer_request_box = SurroundingRectangle(viewer_request_slot, color=SUBREAD_COLOR, buff=0.05, stroke_width=2)
        viewer_request_tag = Text("sub-chunk", font_size=16, color=SUBREAD_COLOR)
        viewer_request_tag.next_to(viewer_request_box, DOWN, buff=0.06)

        viewer_request_slot.set_z_index(20)
        viewer_request_box.set_z_index(21)
        viewer_request_tag.set_z_index(21)

        delivered_slice = self.make_slice(cache.chunk_rect(miss_chunk), 0.5, 0.18, UPSTREAM_COLOR)
        delivered_slice.set_x(cache.body.get_x())
        delivered_slice.set_z_index(22)
        second_slice.set_z_index(22)

        self.play(FadeIn(viewer, shift=LEFT * 0.1), run_time=0.6)
        self.play(
            LaggedStart(
                Create(axis),
                FadeIn(axis_label),
                FadeIn(y_ticks),
                FadeIn(y_labels),
                FadeIn(upstream),
                FadeIn(cache),
                FadeIn(reader_tag),
                FadeIn(writer_tag),
                FadeIn(reader_size),
                FadeIn(writer_size_20),
                lag_ratio=0.05,
            ),
            run_time=0.8,
        )
        self.wait(0.3)

        self.play(
            FadeIn(buffered_badge),
            FadeIn(cache_request_slice),
            FadeOut(viewer.lines),
            FadeIn(viewer_request_slot),
            FadeIn(viewer_request_box),
            FadeIn(viewer_request_tag),
            run_time=1.0,
        )
        self.play(GrowArrow(ask_cache_arrow), run_time=0.8)
        self.play(FadeIn(miss_badge), GrowArrow(miss_to_upstream), run_time=1.1)
        self.play(
            full_chunk_transfer.animate.move_to(cache.chunk_center(miss_chunk)).set_fill(CACHE_COLOR, opacity=0.8).set_stroke(CACHE_COLOR, width=2),
            run_time=1.2,
        )
        self.play(
            GrowArrow(fill_cache_arrow),
            FadeIn(cached_chunk_outline),
            cache.chunk_rect(miss_chunk).animate.set_fill(CACHE_COLOR, opacity=0.8),
            FadeOut(full_chunk_transfer),
            FadeTransform(writer_size_20, writer_size_40),
            run_time=1.05,
        )
        self.play(FadeOut(cache_request_slice), FadeIn(delivered_slice), run_time=0.55)
        self.play(
            GrowArrow(miss_return_to_viewer),
            delivered_slice.animate.move_to(viewer_receive_target),
            run_time=1.1,
        )
        self.play(
            FadeOut(ask_cache_arrow),
            FadeOut(miss_return_to_viewer),
            run_time=0.6,
        )
        self.wait(0.55)
        self.play(FadeOut(delivered_slice), run_time=0.35)
        self.wait(0.3)

        self.play(FadeOut(miss_to_upstream), run_time=0.3)
        self.play(miss_badge.animate(path_arc=-0.35).move_to(miss_story_target.get_center()), run_time=0.5)
        self.wait(0.2)

        self.play(FadeIn(prefetch_badge), run_time=0.5)
        self.play(
            prefetch_chunk_transfer.animate.move_to(cache.chunk_center(prefetch_chunk)).set_fill(CACHE_COLOR, opacity=0.8).set_stroke(CACHE_COLOR, width=2),
            run_time=1.0,
        )
        self.play(
            FadeIn(prefetched_chunk_outline),
            cache.chunk_rect(prefetch_chunk).animate.set_fill(CACHE_COLOR, opacity=0.8),
            FadeOut(prefetch_chunk_transfer),
            FadeTransform(writer_size_40, writer_size_60),
            run_time=0.9,
        )
        self.play(FadeOut(fill_cache_arrow), run_time=0.45)
        self.play(
            prefetch_badge.animate(path_arc=-0.35).move_to(prefetch_story_target.get_center()),
            FadeIn(story_arrow_1),
            run_time=0.5,
        )
        self.wait(0.45)

        self.play(FadeIn(hit_badge), FadeIn(second_slice), run_time=0.9)
        hit_ask_cache_arrow = Arrow(
            buffered_badge.get_left() + UP * 0.1,
            second_slice.get_right() + RIGHT * 0.08,
            buff=0.18,
            color=VIEWER_COLOR,
            stroke_width=4,
        )
        hit_return_arrow = Arrow(
            second_slice.get_right() + RIGHT * 0.08,
            buffered_badge.get_left() + DOWN * 0.1,
            buff=0.18,
            color=CACHE_COLOR,
            stroke_width=4,
        )
        self.play(GrowArrow(hit_ask_cache_arrow), run_time=0.75)
        self.play(GrowArrow(hit_return_arrow), second_slice.animate.move_to(viewer_receive_target), run_time=1.0)
        self.play(FadeOut(hit_ask_cache_arrow), FadeOut(hit_return_arrow), run_time=0.5)
        self.play(hit_badge.animate(path_arc=-0.35).move_to(hit_story_target.get_center()), FadeIn(story_arrow_2), run_time=0.5)
        self.wait(1.2)

    def badge(self, label: str, color: str, text_color: str | None = None, fill_opacity: float = 0.08) -> VGroup:
        text = Text(label, font_size=15, color=text_color or color, weight="BOLD")
        frame = SurroundingRectangle(text, color=color, buff=0.13, corner_radius=0.08)
        frame.set_fill(color, opacity=fill_opacity)
        return VGroup(frame, text)

    def make_slice(self, chunk_rect: Rectangle, center_ratio: float, height_ratio: float, color: str) -> Rectangle:
        slice_rect = Rectangle(
            width=chunk_rect.width * 0.92,
            height=chunk_rect.height * height_ratio,
            stroke_color=color,
            stroke_width=2,
            fill_color=color,
            fill_opacity=0.82,
        )
        top = chunk_rect.get_top()
        bottom = chunk_rect.get_bottom()
        center_y = top[1] + (bottom[1] - top[1]) * center_ratio
        slice_rect.move_to([chunk_rect.get_center()[0], center_y, 0])
        return slice_rect
