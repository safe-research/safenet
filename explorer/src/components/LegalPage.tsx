import { ConditionalBackButton } from "@/components/BackButton";
import { Box, Container, ContainerTitle } from "@/components/Groups";

interface LegalPageProps {
	title: string;
	/** Trusted HTML rendered at build time from the deployment's HTML fragment file. */
	html: string;
}

export function LegalPage({ title, html }: LegalPageProps) {
	return (
		<Container className="space-y-4">
			<ConditionalBackButton />
			<ContainerTitle>{title}</ContainerTitle>
			<Box>
				{/* biome-ignore lint/security/noDangerouslySetInnerHtml: build-time operator content, see vite.config.js */}
				<div className="legal-content" dangerouslySetInnerHTML={{ __html: html }} />
			</Box>
		</Container>
	);
}
