import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { App } from "../src/App";

test("renders the title", () => {
  render(<App />);
  expect(screen.getByRole("heading", { name: "duel-ultimate" })).toBeTruthy();
});
