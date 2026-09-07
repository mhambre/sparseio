from manim import (
    LEFT, RIGHT, WHITE, Arrow, FadeIn, FadeOut, GrowArrow, Indicate,
    Line, Polygon, Rectangle, RoundedRectangle, Scene, Text, VGroup, config,
)


config.background_color = "#07111F"
config.max_files_cached = 200
FONT = "Verdana"
MUTED = "#AAB8C8"
FRAME = "#7F8EA3"
GOLD = "#F6BD60"
CYAN = "#4CC9F0"
GREEN = "#90BE6D"
CONTENT_COLORS = {"A": GREEN, "B": "#F28482", "C": CYAN}


def label(value, size=24, color=WHITE):
    return Text(value, font=FONT, font_size=size, color=color,
                disable_ligatures=True)


def chunk(width=0.55, height=0.55, shared=True, value="A"):
    color = CONTENT_COLORS[value] if shared else FRAME
    body = Rectangle(width=width, height=height, stroke_color=color,
                     stroke_width=1.5, fill_color=color,
                     fill_opacity=0.65 if shared else 0.12)
    if shared:
        content = label(value, 22)
    else:
        content = VGroup(*[
            Line([-width * 0.25, y, 0], [width * 0.25, y, 0],
                 stroke_width=1.5, color=FRAME)
            for y in [-0.09, 0.09]
        ])
    return VGroup(body, content)


class CasScene(Scene):
    """Equal chunks share storage while distinct chunks retain separate entries."""

    def construct(self):
        titles = VGroup(
            label("Upstreams", 27, CYAN).move_to([-4.6, 2.85, 0]),
            label("CAS", 27, GREEN).move_to([0, 2.95, 0]),
            label("(Storage Efficiency)", 16, MUTED).move_to([0, 2.58, 0]),
            label("Viewers", 27, GOLD).move_to([4.6, 2.85, 0]),
        )
        rows = VGroup()
        sources = []
        unique_sources = []
        viewers = VGroup()
        slots = []
        for name, y, unique in zip(["S3", "HTTPS", "Hugging Face"], [1.65, 0, -1.65], [None, "B", "C"]):
            cells = VGroup(*[
                chunk(shared=i == 3 or (i == 2 and unique is not None),
                      value=unique if i == 2 and unique is not None else "A")
                for i in range(4)
            ])
            cells.arrange(RIGHT, buff=0.10).move_to([-4.6, y - 0.08, 0])
            file_outline = Polygon(
                [-6.05, y + 0.60, 0], [-3.40, y + 0.60, 0],
                [-3.15, y + 0.35, 0], [-3.15, y - 0.48, 0],
                [-6.05, y - 0.48, 0],
                stroke_color=CYAN, stroke_width=1.5,
                fill_color=CYAN, fill_opacity=0.03,
            )
            fold = VGroup(
                Line([-3.40, y + 0.60, 0], [-3.40, y + 0.35, 0], color=CYAN, stroke_width=1.5),
                Line([-3.40, y + 0.35, 0], [-3.15, y + 0.35, 0], color=CYAN, stroke_width=1.5),
            )
            filename = label(f"file-{len(sources) + 1}.bin", 17, MUTED)
            filename.move_to([-4.6, y + 0.38, 0])
            source_name = label(name, 20, CYAN).move_to([-4.6, y + 0.84, 0])
            rows.add(VGroup(file_outline, fold, cells, filename, source_name))
            sources.append(cells[-1])
            if unique is not None:
                unique_sources.append((unique, cells[2], len(sources) - 1))
            panel = RoundedRectangle(
                width=2.85, height=1.05, corner_radius=0.12,
                stroke_color=GOLD, stroke_width=2,
                fill_color=GOLD, fill_opacity=0.06,
            ).move_to([4.6, y, 0])
            slot = Rectangle(width=0.6, height=0.6, stroke_color=GOLD,
                             stroke_width=1, fill_opacity=0).move_to(panel)
            viewers.add(VGroup(panel, slot))
            slots.append(slot)

        cache = RoundedRectangle(
            width=2.7, height=4.8, corner_radius=0.14,
            stroke_color=GREEN, stroke_width=2,
            fill_color=GREEN, fill_opacity=0.04,
        )
        cached = chunk(width=1.2, height=1.2).move_to([0, 0.75, 0])
        cached[1].scale(1.6)
        stored = label("0", 30, MUTED).move_to([0, -1.75, 0])
        stored_label = label("stored", 19, MUTED).move_to([0, -2.12, 0])
        self.play(FadeIn(titles), FadeIn(rows), FadeIn(cache),
                  FadeIn(viewers), FadeIn(stored), FadeIn(stored_label), run_time=0.8)
        self.wait(1.0)

        outputs = VGroup()
        for index, (source, viewer, slot) in enumerate(zip(sources, viewers, slots)):
            request = Arrow(viewer.get_left(), cache.get_right(),
                            color=GOLD, buff=0.14, stroke_width=3)
            upstream_request = Arrow(cache.get_left(), source.get_right(),
                                     color=GOLD, buff=0.14, stroke_width=3)
            self.play(GrowArrow(request), Indicate(slot, color=GOLD), run_time=0.55)
            self.play(GrowArrow(upstream_request), run_time=0.4)
            self.play(FadeOut(request), FadeOut(upstream_request), run_time=0.2)

            packet = source.copy().set_z_index(5)
            self.add(packet)
            self.play(packet.animate.move_to(cached).scale(1.2 / 0.55), run_time=1.1)
            if index == 0:
                self.remove(packet)
                self.add(cached)
                one = label("1", 30, GREEN).move_to(stored)
                self.play(FadeOut(stored), FadeIn(one), run_time=0.3)
                stored = one
            else:
                self.play(FadeOut(packet), Indicate(cached, color=GREEN),
                          Indicate(stored, color=GREEN), run_time=0.65)

            delivered = chunk().move_to(cached).set_z_index(5)
            self.add(delivered)
            self.play(delivered.animate.move_to(slot), run_time=1.0)
            outputs.add(delivered)
            self.wait(0.7)

        self.wait(1.4)
        self.play(FadeOut(outputs), run_time=0.25)
        requests = VGroup(*[
            Arrow(viewer.get_left(), cache.get_right(), color=GOLD,
                  buff=0.14, stroke_width=3) for viewer in viewers
        ])
        self.play(*[GrowArrow(request) for request in requests], run_time=0.65)
        self.play(FadeOut(requests), Indicate(cached, color=GREEN), run_time=0.45)
        fanout_label = label("Deduplicated fanout", 18, GREEN).move_to([3.65, 2.4, 0])
        fanout = VGroup(*[
            Arrow(cached.get_right(), viewer.get_left(), color=GREEN,
                  buff=0.12, stroke_width=2.5) for viewer in viewers
        ])
        self.play(FadeIn(fanout_label), *[GrowArrow(branch) for branch in fanout], run_time=0.5)
        replies = VGroup(*[chunk().move_to(cached).set_z_index(5) for _ in slots])
        self.add(replies)
        self.play(*[reply.animate.move_to(slot) for reply, slot in zip(replies, slots)], run_time=1.2)
        self.wait(1.2)
        self.play(FadeOut(fanout), FadeOut(fanout_label), run_time=0.3)

        for index, (value, source, viewer_index) in enumerate(unique_sources):
            color = CONTENT_COLORS[value]
            slot = slots[viewer_index]
            target_slot = slot.copy().shift(RIGHT * 0.43)
            request = Arrow(viewers[viewer_index].get_left(), cache.get_right(),
                            color=GOLD, buff=0.14, stroke_width=3)
            upstream_request = Arrow(cache.get_left(), source.get_right(),
                                     color=GOLD, buff=0.14, stroke_width=3)
            self.play(slot.animate.shift(LEFT * 0.43),
                      replies[viewer_index].animate.shift(LEFT * 0.43),
                      FadeIn(target_slot), GrowArrow(request), run_time=0.55)
            self.play(GrowArrow(upstream_request), run_time=0.4)
            self.play(FadeOut(request), FadeOut(upstream_request), run_time=0.2)
            entry = chunk(width=0.95, height=0.95, value=value)
            entry.move_to([-0.62 + index * 1.24, -0.65, 0])
            packet = source.copy().set_z_index(5)
            self.add(packet)
            self.play(packet.animate.move_to(entry).scale(0.95 / 0.55), run_time=1.1)
            self.remove(packet)
            self.add(entry)
            count = label(str(index + 2), 30, GREEN).move_to(stored)
            self.play(FadeOut(stored), FadeIn(count), Indicate(entry, color=color), run_time=0.4)
            stored = count
            delivered = chunk(value=value).move_to(entry).set_z_index(5)
            self.add(delivered)
            self.play(delivered.animate.move_to(target_slot), run_time=1.0)
            self.wait(0.6)

        result = VGroup(
            label("5", 34, CYAN).move_to([-0.85, -3.05, 0]),
            Arrow([-0.35, -3.05, 0], [0.35, -3.05, 0],
                  buff=0, color=GREEN, stroke_width=3),
            label("3", 34, GREEN).move_to([0.85, -3.05, 0]),
        )
        self.play(FadeIn(result), run_time=0.5)
        self.wait(3.0)
        self.play(*[FadeOut(mob) for mob in list(self.mobjects)], run_time=0.6)
        self.wait(0.3)
