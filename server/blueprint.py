#!/usr/bin/env python3

import argparse
from dataclasses import dataclass
from pathlib import Path

import rerun as rr
import rerun.blueprint as rrb


def create_blueprint(application_id: str, output_path: Path | None):
    # TODO: limit timeline by default to last minute
    blueprint = rrb.Blueprint(
        rrb.Horizontal(
            rrb.Spatial3DView(name="3D", origin="world"),
            rrb.Vertical(
                rrb.Tabs(
                    rrb.Spatial2DView(
                        name="RGB & Depth",
                        origin="world",
                        overrides={"world/camera/rgb": [rr.components.Opacity(0.5)]},
                    ),
                    rrb.Spatial2DView(
                        name="RGB",
                        origin="world/camera/rgb",
                        contents="world/camera/rgb",
                    ),
                    rrb.Spatial2DView(
                        name="Depth",
                        origin="world/lidar",
                        contents="world/lidar",
                    ),
                ),
                rrb.Tabs(
                    rrb.TextLogView(
                        name="Command History",
                        origin="commands",
                        contents="commands",
                    ),
                    rrb.TextDocumentView(
                        name="Depth",
                        origin="description",
                        contents="description",
                    ),
                ),
                row_shares=[4, 2],
            ),
            column_shares=[2, 1],
        ),
        auto_views=False,
    )

    blueprint.save(application_id, None if output_path is None else str(output_path))


@dataclass
class Arguments(argparse.Namespace):
    application_id: str
    output_path: Path


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    _ = parser.add_argument(
        "--output-path", type=Path, default=Path(Path(__file__).parent, "blueprint.rbl")
    )
    _ = parser.add_argument("-a", "--application-id", type=str, default="brainlesscar")
    args = parser.parse_args(namespace=Arguments)
    create_blueprint(args.application_id, args.output_path)
