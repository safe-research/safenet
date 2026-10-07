import { createFileRoute, notFound } from "@tanstack/react-router";
import { LegalPage } from "@/components/LegalPage";

export const Route = createFileRoute("/imprint")({
	beforeLoad: () => {
		if (!__IMPRINT_HTML__) {
			throw notFound();
		}
	},
	component: () => <LegalPage title="Imprint" html={__IMPRINT_HTML__} />,
});
