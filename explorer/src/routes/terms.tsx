import { createFileRoute, notFound } from "@tanstack/react-router";
import { LegalPage } from "@/components/LegalPage";

export const Route = createFileRoute("/terms")({
	beforeLoad: () => {
		if (!__TERMS_HTML__) {
			throw notFound();
		}
	},
	component: () => <LegalPage title="Terms" html={__TERMS_HTML__} />,
});
