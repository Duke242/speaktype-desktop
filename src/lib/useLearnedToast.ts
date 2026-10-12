import { useCallback } from "react";
import { useToast } from "@/components/ui";
import { api, errorMessage, type DictionaryEntry } from "@/lib/api";

/** "Learned “Karan”", or "Learned “Karan” and 2 more". */
export function learnedMessage(entries: DictionaryEntry[]) {
  const [first, ...rest] = entries;
  if (!first) return "";
  const word = `Learned “${first.replacement}”`;
  return rest.length > 0 ? `${word} and ${rest.length} more` : word;
}

/** Tells the user which words were learned from a correction, with Undo. */
export function useLearnedToast() {
  const toast = useToast();
  return useCallback(
    (entries: DictionaryEntry[]) => {
      if (entries.length === 0) return;
      toast(learnedMessage(entries), "success", {
        label: "Undo",
        onClick: () =>
          Promise.all(entries.map((entry) => api.forgetWord(entry.id))).catch((e) =>
            toast(errorMessage(e), "error"),
          ),
      });
    },
    [toast],
  );
}
