import { Card, Col, Row, Statistic } from 'antd';
import {
  ApiOutlined,
  DatabaseOutlined,
  CloudServerOutlined,
  ArrowUpOutlined,
} from '@ant-design/icons';
import { useGetApisQuery, useGetDataSourcesQuery, useGetListenersQuery } from '@/services/api';

export const Dashboard = () => {
  const { data: apis = [] } = useGetApisQuery();
  const { data: dataSources = [] } = useGetDataSourcesQuery();
  const { data: listeners = [] } = useGetListenersQuery();

  return (
    <div>
      <h1 style={{ marginBottom: 24 }}>仪表板</h1>
      <Row gutter={[16, 16]}>
        <Col xs={24} sm={12} lg={8}>
          <Card>
            <Statistic
              title="API 配置"
              value={apis.length}
              prefix={<ApiOutlined />}
              valueStyle={{ color: '#3f8600' }}
              suffix={<ArrowUpOutlined />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={8}>
          <Card>
            <Statistic
              title="数据源"
              value={dataSources.length}
              prefix={<DatabaseOutlined />}
              valueStyle={{ color: '#1890ff' }}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={8}>
          <Card>
            <Statistic
              title="监听器"
              value={listeners.length}
              prefix={<CloudServerOutlined />}
              valueStyle={{ color: '#cf1322' }}
            />
          </Card>
        </Col>
      </Row>

      <Row gutter={[16, 16]} style={{ marginTop: 24 }}>
        <Col span={24}>
          <Card title="快速开始" bordered={false}>
            <div style={{ padding: '20px 0' }}>
              <h3>欢迎使用 Apify Admin Dashboard!</h3>
              <p>这是一个零代码 CRUD API 管理平台。</p>
              <ul style={{ textAlign: 'left', marginLeft: 40 }}>
                <li>配置数据源连接</li>
                <li>创建 API 配置</li>
                <li>设置监听器</li>
                <li>自动生成 RESTful API</li>
              </ul>
            </div>
          </Card>
        </Col>
      </Row>
    </div>
  );
};
