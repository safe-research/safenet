import { createFileRoute, notFound } from "@tanstack/react-router";
import { LegalPage } from "@/components/LegalPage";

export const Route = createFileRoute("/privacy")({
	beforeLoad: () => {
		if (!__PRIVACY_HTML__) {
			throw notFound();
		}
	},
	component: () => <LegalPage title="Privacy" html={__PRIVACY_HTML__} />,
});
