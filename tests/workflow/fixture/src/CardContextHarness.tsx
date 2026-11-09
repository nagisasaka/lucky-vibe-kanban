import { useState } from "react";
import type { Pipeline } from "shared/types";
import { CardContextSection } from "../../../../packages/web-core/src/features/pipeline/ui/CardContextSection";
import {
  defaultCardContext,
  replaceCardDescription,
  splitCardContext,
} from "../../../../packages/web-core/src/features/pipeline/model/cardContext";
import { composePipelineBlock } from "../../../../packages/web-core/src/features/pipeline/model/cardPipeline";

const pipeline: Pipeline = {
  id: "quality",
  name: "Quality checks",
  description: null,
  stages: [
    {
      id: "inspect",
      label: "Inspect",
      prompt_fragment: "Inspect the proposed change.",
      default_enabled: true,
      heavy: false,
    },
  ],
};

export function CardContextHarness() {
  const stored = new URLSearchParams(window.location.search).has(
    "stored-pipeline",
  );
  const [description, setDescription] = useState(() =>
    stored
      ? `Existing task\n\n${composePipelineBlock(pipeline, ["inspect"])}`
      : defaultCardContext("New task", [pipeline]),
  );
  return (
    <main>
      <textarea
        aria-label="Task description"
        value={splitCardContext(description).description}
        onChange={(event) =>
          setDescription(
            replaceCardDescription(description, event.target.value),
          )
        }
      />
      <CardContextSection
        description={description}
        pipelines={[pipeline]}
        onRetry={() => {}}
        onDescriptionChange={setDescription}
      />
      <output data-testid="stored-description">{description}</output>
    </main>
  );
}
