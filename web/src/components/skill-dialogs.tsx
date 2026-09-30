import { useEffect, useMemo, useState } from "react";
import { useForm } from "react-hook-form";
import { useNavigate } from "@tanstack/react-router";
import type { SkillSource } from "@trytilde/contracts/tilde/types/v1/skill_pb.js";
import { skills } from "@/client";
import {
  ENTRYPOINT,
  GroupFields,
  NEW_GROUP,
  chosenGroup,
  message,
  skillTemplate,
  validName,
  type GroupChoice,
} from "./skill-common";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";

type GitForm = { name: string; repositoryUrl: string; gitRef: string; gitPath: string };
/** Adds a Git group, then opens it. */
export function GitSourceDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const navigate = useNavigate();
  const [error, setError] = useState("");
  const form = useForm<GitForm>({
    defaultValues: { name: "", repositoryUrl: "", gitRef: "main", gitPath: "" },
  });
  const { errors, isSubmitting } = form.formState;
  async function submit(values: GitForm) {
    setError("");
    try {
      const { source } = await skills.addGitSource({
        name: values.name.trim(),
        repositoryUrl: values.repositoryUrl.trim(),
        gitRef: values.gitRef.trim() || "main",
        gitPath: values.gitPath.trim(),
      });
      form.reset();
      onOpenChange(false);
      if (source) await navigate({ to: "/skills" });
    } catch (e) {
      setError(message(e, "Unable to add the repository."));
    }
  }
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Add from Git</DialogTitle>
          <DialogDescription>
            Every folder with a SKILL.md in the repository becomes a skill. Tilde syncs it hourly,
            or when you ask.
          </DialogDescription>
        </DialogHeader>
        <form className="grid gap-4" onSubmit={form.handleSubmit(submit)}>
          <div className="grid gap-2">
            <Label htmlFor="git-name">Name</Label>
            <Input
              id="git-name"
              placeholder="Support playbooks"
              aria-invalid={!!errors.name}
              {...form.register("name", { validate: (v) => !!v.trim() || "Enter a name." })}
            />
            {errors.name && <p className="m-0 text-xs text-destructive">{errors.name.message}</p>}
          </div>
          <div className="grid gap-2">
            <Label htmlFor="git-url">GitHub repository URL</Label>
            <Input
              id="git-url"
              placeholder="https://github.com/acme/skills"
              aria-invalid={!!errors.repositoryUrl}
              {...form.register("repositoryUrl", {
                validate: (v) =>
                  /^https:\/\/github\.com\/[^/\s]+\/[^/\s]+\/?$/.test(v.trim()) ||
                  "Enter a https://github.com/<owner>/<repository> URL.",
              })}
            />
            {errors.repositoryUrl && (
              <p className="m-0 text-xs text-destructive">{errors.repositoryUrl.message}</p>
            )}
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div className="grid gap-2">
              <Label htmlFor="git-ref">Branch</Label>
              <Input id="git-ref" {...form.register("gitRef")} />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="git-path">Path (optional)</Label>
              <Input id="git-path" placeholder="skills/" {...form.register("gitPath")} />
            </div>
          </div>
          {error && (
            <p role="alert" className="m-0 text-sm text-destructive">
              {error}
            </p>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={isSubmitting}>
              Add repository
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

type EditorForm = GroupChoice & { name: string };
/**
 * A new skill goes into an existing editor group, or a new group created for it; its
 * starter SKILL.md opens for editing.
 */
export function EditorSkillDialog({
  open,
  groups,
  group,
  onOpenChange,
}: {
  open: boolean;
  groups: SkillSource[];
  /** Preselected group, when opened from its row. */
  group?: string;
  onOpenChange: (open: boolean) => void;
}) {
  const navigate = useNavigate();
  const [created, setCreated] = useState<SkillSource[]>([]);
  const [error, setError] = useState("");
  const form = useForm<EditorForm>({
    defaultValues: { groupId: NEW_GROUP, groupName: "", name: "" },
  });
  const { errors, isSubmitting } = form.formState;
  const { reset, getFieldState, getValues, setValue } = form;
  const options = useMemo(() => [...groups, ...created], [groups, created]);
  useEffect(() => {
    if (!open) return;
    setError("");
    reset({ groupId: group ?? NEW_GROUP, groupName: "", name: "" });
  }, [open, group, reset]);
  // Editable groups load after the page; default to the first unless one was chosen.
  useEffect(() => {
    if (
      open &&
      groups.length &&
      getValues("groupId") === NEW_GROUP &&
      !getFieldState("groupId").isDirty
    )
      setValue("groupId", groups[0].id);
  }, [open, groups, getValues, getFieldState, setValue]);
  async function submit(values: EditorForm) {
    setError("");
    try {
      const target = await chosenGroup(form, values);
      if (target.created) setCreated((current) => [...current, target.created]);
      const name = values.name.trim();
      const { skill } = await skills.createSkill({
        sourceId: target.id,
        name,
        files: [{ path: ENTRYPOINT, content: skillTemplate(name) }],
      });
      onOpenChange(false);
      if (skill) await navigate({ to: "/skills/$skillId", params: { skillId: skill.id } });
    } catch (e) {
      setError(message(e, "Unable to create the skill."));
    }
  }
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Create in editor</DialogTitle>
          <DialogDescription>
            The name becomes the skill's folder. A starter SKILL.md is created for you to edit.
          </DialogDescription>
        </DialogHeader>
        <form className="grid gap-4" onSubmit={form.handleSubmit(submit)}>
          <GroupFields form={form} groups={options} />
          <div className="grid gap-2">
            <Label htmlFor="editor-skill-name">Skill name</Label>
            <Input
              id="editor-skill-name"
              placeholder="refund-policy"
              aria-invalid={!!errors.name}
              {...form.register("name", {
                validate: (v) =>
                  validName(v.trim()) || "Skill names use lowercase letters, digits, '-' or '_'.",
              })}
            />
            {errors.name && <p className="m-0 text-xs text-destructive">{errors.name.message}</p>}
          </div>
          {error && (
            <p role="alert" className="m-0 text-sm text-destructive">
              {error}
            </p>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={isSubmitting}>
              Create skill
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
