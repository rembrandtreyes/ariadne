import { formatDate } from '../lib/utils';

function Timestamp({ value }: { value: string }) {
    return <time>{value}</time>;
}

export default function Page() {
    return <Timestamp value={formatDate(new Date())} />;
}

export function generateMetadata() {
    return { title: 'Home' };
}
