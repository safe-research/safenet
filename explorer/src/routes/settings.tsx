import { createFileRoute } from "@tanstack/react-router";
import { ConditionalBackButton } from "@/components/BackButton";
import { Box, BoxTitle, Container, ContainerTitle } from "@/components/Groups";
import { ConsensusSettingsForm } from "@/components/settings/ConsensusSettingsForm";
import { UiSettingsForm } from "@/components/settings/UiSettingsForm";

export const Route = createFileRoute("/settings")({
	component: SettingsPage,
});

export function SettingsPage() {
	return (
		<Container className="space-y-4">
			<ConditionalBackButton />
			<ContainerTitle>Settings</ContainerTitle>
			<Box>
				<BoxTitle>UI Settings</BoxTitle>
				<UiSettingsForm />
			</Box>
			<Box>
				<BoxTitle>Consensus Settings</BoxTitle>
				<ConsensusSettingsForm />
			</Box>
		</Container>
	);
}
